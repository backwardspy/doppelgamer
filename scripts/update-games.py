import json
import urllib.request
from pathlib import Path

URL = "https://discord.com/api/v9/applications/detectable"
HEADERS = {
    "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
    "Accept-Language": "en-GB,en;q=0.9",
    "Connection": "keep-alive",
    "Host": "discord.com",
    "Priority": "u=0, i",
    "Sec-Fetch-Dest": "document",
    "Sec-Fetch-Mode": "navigate",
    "Sec-Fetch-Site": "none",
    "Sec-Fetch-User": "?1",
    "Sec-GPC": "1",
    "TE": "trailers",
    "Upgrade-Insecure-Requests": "1",
    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:147.0) Gecko/20100101 Firefox/147.0",
}

detectable = json.loads(
    urllib.request.urlopen(urllib.request.Request(URL, headers=HEADERS))
    .read()
    .decode("utf-8")
)

games = []

for game in detectable:
    if any(x["os"] == "win32" for x in game["executables"]):
        exe = next(
            (
                x
                for x in game["executables"]
                if x["os"] == "win32" and not x["is_launcher"]
            ),
            None,
        )
        if exe is None:
            exe = next((x for x in game["executables"] if x["os"] == "win32"), None)
        if exe is None:
            print("warn: somehow couldn't find valid exe for game", game["name"])
            continue
        games.append(
            {
                "name": game["name"],
                "exe": exe["name"],
            }
        )

with Path("games.v2.json").open("w", encoding="utf-8") as f:
    json.dump(games, f, ensure_ascii=False)
