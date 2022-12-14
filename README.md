# GitHub Research Runner (ghrr)
A utility to collect data from github stargazers, subscribers and contributors of a selected project.

Usecases are detailed at the following blog: https://www.battery.com/blog/fantastic-developer-community-heroes-and-where-to-find-them/

## installing

```bash

git clone https://github.com/schosterbarak/ghrr.git
cd ghrr
python setup.py install

```

## Using
1. Create github personal access token using the following guide:
https://docs.github.com/en/github/authenticating-to-github/creating-a-personal-access-token

2. Run:
```bash
export GITHUB_USER=XXX
export GITHUB_TOKEN=YYY

#an example command 
ghrr --org bridgecrewio --repo checkov 

```

## Result
will be created at the following format under the working directory:
`ghusers_{ORG}_{REPO}_{DATE}.csv`

example:
`ghusers_bridgecrewio_checkov_2020-09-21.csv`
`
| username   | company  | organizations   | email           | location    | followers_count   | public_repos_count   | user_iteraction   |
| --------   | -------  | -------------   | -------         | --------    | ---------------   | ------------------   | ---------------   |
| Jon.       | ACME.    | ACME.           | jon@acme.com    | US          | 3                 | 200                  | stargazer   |
| Jane       | ACME.    | ACME.          | jane@acme.com    | IL          | 3                 | 200                  | collaborator   |

`

When loading to a BI tool:
![GitHub Influencers Dashboard](/influencers%20dashboard.png "GitHub Influencers Dashboard").

