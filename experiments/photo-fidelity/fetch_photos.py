#!/usr/bin/env python3
"""Download the pinned public photo corpus, checking every file's SHA-256.

Original source, author and license-as-reported-by-Commons are in photos.json.
Images go under target/ and are not included in the repository.
"""
import argparse
import hashlib
import json
from pathlib import Path
import time
import urllib.error
import urllib.request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("out", nargs="?", type=Path, default=Path("target/photo-fidelity-corpus"))
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    rows = json.loads(Path(__file__).with_name("photos.json").read_text())
    for row in rows:
        path = args.out / row["file"]
        if path.exists():
            content = path.read_bytes()
        else:
            request = urllib.request.Request(row["download_url"], headers={
                "User-Agent": "ChromoxidePhotoFidelity/0.1 (local research comparison)",
            })
            for attempt in range(4):
                try:
                    with urllib.request.urlopen(request, timeout=45) as response:
                        content = response.read()
                    break
                except urllib.error.HTTPError as error:
                    if error.code != 429 or attempt == 3:
                        raise
                    time.sleep(max(10, int(error.headers.get("Retry-After", 30))))
            time.sleep(1)
        if hashlib.sha256(content).hexdigest() != row["sha256"]:
            raise ValueError(f"Photo changed: {path}; keep the pinned manifest and investigate")
        if not path.exists():
            path.write_bytes(content)
        print(f"Verified {path}")


if __name__ == "__main__":
    main()
