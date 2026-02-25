import json
import urllib.request
from pathlib import Path

BADWORDS_URL = "https://raw.githubusercontent.com/LDNOOBW/List-of-Dirty-Naughty-Obscene-and-Otherwise-Bad-Words/refs/heads/master/en"
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
EXTRA_GAMES = [
    {"name": "Opera GX", "exe": "Opera GX/opera.exe"},
]

badwords = [
    line.strip().lower()
    for line in urllib.request.urlopen(BADWORDS_URL).read().decode("utf-8").splitlines()
    if line.strip()
]

detectable = json.loads(
    urllib.request.urlopen(urllib.request.Request(URL, headers=HEADERS))
    .read()
    .decode("utf-8")
)

games = []
filtered = 0

for game in detectable:
    name_lower = game["name"].lower()
    if any(word in name_lower for word in badwords):
        filtered += 1
        continue
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

print(f"filtered {filtered} games via badwords list")

existing_exes = {g["exe"] for g in games}
extra_added = 0
for g in EXTRA_GAMES:
    if g["exe"] not in existing_exes:
        games.append(g)
        existing_exes.add(g["exe"])
        extra_added += 1
    else:
        print(f"warn: extra game {g['name']} is already in the list, skipping")
print(f"added {extra_added} extra games")


with Path("games.v2.json").open("w", encoding="utf-8") as f:
    json.dump(games, f, ensure_ascii=False)
