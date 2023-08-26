from fastapi import FastAPI
import json
from twitter.scraper import Scraper

with open("creds.json") as f:
    creds = json.load(f)

app = FastAPI()

scraper = Scraper(creds["username"], creds["password"])


@app.get("/{twitter_id}")
def get_followers(twitter_id):
    try:
        user = scraper.user_by_rest_id([twitter_id])
        print(user)
        return user[0]["data"]["user"]["result"]["legacy"]["followers_count"]

    except Exception as e:
        print(e)
        return 0
