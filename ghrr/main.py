import argparse
import csv
import os, sys
from datetime import date, datetime
from collections import namedtuple
from time import sleep
from github3 import login
from github3.exceptions import ForbiddenError
from tqdm import tqdm


RATE_LIMIT_BACKOFF= 10
gh_user = os.getenv('GITHUB_USER')
gh_token = os.getenv('GITHUB_TOKEN')

parser = argparse.ArgumentParser(description='stargazers crawler')
parser.add_argument('-o', '--organization', action='store',
                    help='github organization', required=True)
parser.add_argument('-r', '--repository', action='store',
                    help='github repository', required=True)
parser.add_argument('-f', '--file', action='store',
                    help='output file path', required=False)

warn = lambda msg: print(f'\033[93mError: {msg}\033[0m', file=sys.stderr)
die = lambda msg: warn(msg) or exit(1)

User = namedtuple('User', ['email', 'location','username','company','followers','repos','organizations'])

def get_user_data(gh, u):
    username = u.login
    gh_user = gh.user(username)
    company = gh_user.company
    followers = gh_user.followers_count
    repos = gh_user.public_repos_count
    if company:
        company = company.replace("@", "")
    organizationsIterator = gh_user.organizations()
    organizations = []
    for org in organizationsIterator:
        organizations.append(org.login)
    return User(gh_user.email, gh_user.location, username, company, followers, repos, organizations)

def validate_params(args):
    if not gh_user:
        die("Please add GITHUB_USER environment variable")
    if not gh_token:
        die("Please add GITHUB_TOKEN environment variable", file=os.Stderr)
    if not args.repository and not args.organization:
        die("No argument given. Try ` --help` for further information")


class DummyUpdater(object):

    def update(self, c):
        pass

class DummyProgress(object):
    def __init__(self, *args, **kwargs):
        pass
    def __enter__(self):
        return DummyUpdater()
    def __exit__(self, type, value, traceback):
        pass


def wait_rate_limit(e: ForbiddenError):
    if not e.message.startswith('API rate limit exceeded'):
        raise e
    reset_time = e.response.headers.get('X-RateLimit-Reset')
    sleep_until = datetime.fromtimestamp(int(reset_time))
    # add 3 seconds to compensate for clocks being clocks
    seconds_to_sleep =  int((sleep_until - datetime.now()).total_seconds()) + 3  
    sleep_until_pretty = sleep_until.strftime('%m/%d/%Y, %H:%M:%S')
    warn(f'Rate limited. sleeping until "{sleep_until_pretty}" due to rate limit...')
    sleep(seconds_to_sleep)


def iterate_users(gh, users_iterator, users_count, user_writer,user_interaction, progress=True):
    total = users_count if users_count > 0 else None
    progress = tqdm(total=total, desc=f'Fetching {user_interaction} data', unit='users') if progress else DummyProgress()
    with progress as progress_bar:
        for u in users_iterator:
            data_received = False
            while not data_received:
                try:
                    user = get_user_data(gh, u)
                    user_writer.writerow([
                        user.username,
                        user.company, 
                        user.organizations, 
                        user.email, 
                        user.location, 
                        user.followers,
                        user.repos,
                        user_interaction
                    ])
                    data_received = True
                    progress_bar.update(1) 
                except ForbiddenError as e:
                    wait_rate_limit(e)
                    continue
                except Exception as ae:
                    warn(f'got an unexpected error: {ae}, will wait a bit and try again')
                    sleep(RATE_LIMIT_BACKOFF)


if __name__ == '__main__':
    args = parser.parse_args()
    validate_params(args)
    repository = args.repository
    org = args.organization
    gh = login(gh_user, token=gh_token)
    repo_succeeded = False
    output = None
    progress = True
    while not repo_succeeded:
        try:
            gh_repository = gh.repository(org, repository)
            stargazers = gh_repository.stargazers()
            stargazers_count = gh_repository.stargazers_count
            contributors = gh_repository.contributors()
            subscribers = gh_repository.subscribers()
            subscribers_count = gh_repository.subscribers_count
            if not args.file:
                today = date.today()
                formated_today = today.strftime("%Y-%m-%d")  # YY-MM-DD
                file_name = 'ghusers_{}_{}_{}.csv'.format(org, repository, formated_today)
                output = open(file_name, mode='w')
            elif args.file == '-':
                output = sys.stdout
                progress = False
            else:
                output = open(args.file, mode='w')
            repo_succeeded = True
        except ForbiddenError as e:
            wait_rate_limit(e)
            continue
        except Exception as ae:
            warn(f'got an unexpected error: {ae}, will wait a bit and try again')
            sleep(RATE_LIMIT_BACKOFF)

    with output as out:
        user_writer = csv.writer(out)
        user_writer.writerow(["username", "company", "organizations", "email", "location","followers_count","public_repos_count", "user_interaction"])
        iterate_users(gh, stargazers, stargazers_count, user_writer,"stargazer", progress=progress)
        iterate_users (gh, subscribers, subscribers_count, user_writer,"subscriber", progress=progress)
        iterate_users (gh, contributors, -1, user_writer,"contributor", progress=progress)
