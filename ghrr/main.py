import argparse
import csv
import os
from datetime import date, datetime
from time import sleep
import requests
from lxml import html
from github3 import login
from tqdm import tqdm

RATE_LIMIT_BACKOFF= 60 * 15
gh_user = os.getenv('GITHUB_USER')
gh_token = os.getenv('GITHUB_TOKEN')

parser = argparse.ArgumentParser(description='stargazers crawler')
parser.add_argument('-o', '--organization', action='store',
                    help='github organization', required=True)
parser.add_argument('-r', '--repository', action='store',
                    help='github repository', required=True)
args = parser.parse_args()

def get_user_data(gh, u):
    username = u.login
    gh_user = gh.user(username)
    company = gh_user.company
    gh_user_followers = gh_user.followers_count
    gh_user_repos=gh_user.public_repos_count
    if company:
        company = company.replace("@", "")
    organizationsIterator = gh_user.organizations()
    organizations = []
    for org in organizationsIterator:
        organizations.append(org.login)
    return gh_user,username,company,gh_user_followers,gh_user_repos,organizations


def validate_params():
    if not gh_user:
        print("Please add GITHUB_USER environment variable")
        exit(1)
    if not gh_token:
        print("Please add GITHUB_TOKEN environment variable")
        exit(1)
    if not args.repository and not args.organization:
        print("No argument given. Try ` --help` for further information")
        exit(1)


validate_params()

repository = args.repository
org = args.organization
gh = login(gh_user, token=gh_token)
repo_succeeded = False
while not repo_succeeded:
    try:
        gh_repository = gh.repository(org, repository)
        stargazers = gh_repository.stargazers()
        stargazers_count = gh_repository.stargazers_count
        contributors = gh_repository.contributors()
        subscribers = gh_repository.subscribers()
        subscribers_count = gh_repository.subscribers_count
        today = date.today()
        formated_today = today.strftime("%Y-%m-%d")  # YY-MM-DD
        file_name = 'ghusers_{}_{}_{}.csv'.format(org, repository, formated_today)
        repo_succeeded = True
    except Exception:
        # now = datetime.now() 
        # current_time = now.strftime("%H:%M:%S")
        # print(
        #     "repo {} Rate limit occurred. Taking a step back and going to sleep for 15 minutes. This might take an hour up until the next batch.".format(
        #         current_time))
        sleep(RATE_LIMIT_BACKOFF)

print("Starting to collect data. This takes time. \nMake some coffee :) ")



def iterate_users(gh, users_iterator, users_count, user_writer,user_iteraction):
    if users_count > 0:
        with tqdm(total = users_count, desc='Fetching {}s data'.format(user_iteraction)) as progress_bar:
            for u in tqdm(users_iterator):
                data_received = False
                while not data_received:
                    try:
                        gh_user, username, company, gh_user_followers, gh_user_repos, organizations = get_user_data(gh, u)
                        user_writer.writerow([username, company, organizations, gh_user.email, gh_user.location, gh_user_followers,gh_user_repos,user_iteraction])
                        data_received = True
                        progress_bar.update(1) 
                    except Exception:
                        sleep(RATE_LIMIT_BACKOFF)
    if users_count == -1:
        for u in tqdm(users_iterator,  desc='Fetching {}s data'.format(user_iteraction)):
            data_received = False
            while not data_received:
                try:
                    gh_user, username, company, gh_user_followers, gh_user_repos, organizations = get_user_data(gh, u)
                    user_writer.writerow([username, company, organizations, gh_user.email, gh_user.location, gh_user_followers,gh_user_repos,user_iteraction])
                    data_received = True
                except Exception:
                    sleep(RATE_LIMIT_BACKOFF)

with open(file_name, mode='w') as employee_file:
    user_writer = csv.writer(employee_file)
    user_writer.writerow(["username", "company", "organizations", "email", "location","followers_count","public_repos_count", "user_iteraction"])
    iterate_users( gh, stargazers, stargazers_count, user_writer,"stargazer")
    iterate_users (gh, subscribers, subscribers_count, user_writer,"subscriber")
    iterate_users (gh, contributors, -1, user_writer,"contributor")
    print("Results availble at: {}".format(file_name))