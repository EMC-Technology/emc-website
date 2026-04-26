"""GEMMA 4.0 E4B Model Downloader - Mirror Version"""
import os
import sys
from pathlib import Path
import urllib.request
import ssl
import json
import time
import hashlib

MIRROR_URL = "https://hf-mirror.com"
REPO_ID = "google/gemma-4-e4b-it"
TARGET_DIR = Path(r"D:\WorkSpace\1文本全结构化知识系统.md\crates\knowledge-api\data\models\google_gemma-4-e4b-it")

REQUIRED_FILES = [
    "config.json",
    "tokenizer.json",
    "tokenizer.config.json",
]

def download_file(url, dest_path, desc=""):
    """Download a single file with progress indication"""
    print(f"  Downloading {desc}...", end=" ", flush=True)

    ctx = ssl.create_default_context()
    ctx.check_hostname = False
    ctx.verify_mode = ssl.CERT_NONE

    try:
        req = urllib.request.Request(url)
        req.add_header('User-Agent', 'Mozilla/5.0')

        with urllib.request.urlopen(req, timeout=300, context=ctx) as response:
            data = response.read()

            with open(dest_path, 'wb') as f:
                f.write(data)

            size_mb = len(data) / (1024 * 1024)
            if size_mb >= 1024:
                print(f"OK ({size_mb/1024:.2f} GB)")
            else:
                print(f"OK ({size_mb:.2f} MB)")

            return True

    except Exception as e:
        print(f"FAILED ({e})")
        return False

def main():
    print("=" * 70)
    print(" GEMMA 4.0 4B Model Downloader (Mirror)")
    print("=" * 70)
    print()
    print(f"Mirror:     {MIRROR_URL}")
    print(f"Repository: {REPO_ID}")
    print(f"Target:     {TARGET_DIR}")
    print()

    TARGET_DIR.mkdir(parents=True, exist_ok=True)

    start_time = time.time()
    success_count = 0
    fail_count = 0

    # Step 1: Download config and small files
    print("[Step 1/3] Downloading configuration files...")
    print("-" * 50)

    for filename in REQUIRED_FILES:
        url = f"{MIRROR_URL}/{REPO_ID}/resolve/main/{filename}"
        dest = TARGET_DIR / filename

        if download_file(url, dest, filename):
            success_count += 1
        else:
            fail_count += 1

    print()

    # Step 2: Find and download model weights (safetensors files)
    print("[Step 2/3] Discovering model weight files...")

    try:
        # Try to get the list of files from the repo
        index_url = f"{MIRROR_URL}/api/models/{REPO_ID}"
        req = urllib.request.Request(index_url)
        req.add_header('User-Agent', 'Mozilla/5.0')

        ctx = ssl.create_default_context()
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE

        with urllib.request.urlopen(req, timeout=30, context=ctx) as response:
            repo_info = json.loads(response.read())

            siblings = repo_info.get('siblings', [])
            safetensors_files = [s for s in siblings if s['rfilename'].endswith('.safetensors')]

            if not safetensors_files:
                print("  No .safetensors files found in repository!")
                print("  The model may use a different format.")
            else:
                print(f"  Found {len(safetensors_files)} weight file(s):")
                for sf in safetensors_files:
                    size_str = ""
                    if sf.get('lfs', {}).get('size'):
                        size_gb = sf['lfs']['size'] / (1024**3)
                        size_str = f" ({size_gb:.2f} GB)"
                    print(f"    - {sf['rfilename']}{size_str}")

                print()
                print("-" * 50)
                print("[Step 3/3] Downloading model weights (this may take a while)...")
                print("-" * 50)

                for sf in safetensors_files:
                    filename = sf['rfilename']
                    url = f"{MIRROR_URL}/{REPO_ID}/resolve/main/{filename}"
                    dest = TARGET_DIR / filename

                    if download_file(url, dest, filename):
                        success_count += 1
                    else:
                        fail_count += 1

    except Exception as e:
        print(f"  Could not auto-discover files: {e}")
        print()
        print("  Manual fallback: You can download files directly from:")
        print(f"  {MIRROR_URL}/{REPO_ID}/tree/main")
        fail_count += 1

    # Summary
    elapsed = time.time() - start_time

    print()
    print("=" * 70)
    print(" DOWNLOAD SUMMARY")
    print("=" * 70)
    print(f"Time elapsed:   {elapsed:.0f}s ({elapsed/60:.1f} min)")
    print(f"Successful:     {success_count} file(s)")
    print(f"Failed:         {fail_count} file(s)")

    if TARGET_DIR.exists():
        files = list(TARGET_DIR.iterdir())
        if files:
            total_size = sum(f.stat().st_size for f in files if f.is_file())
            total_gb = total_size / (1024**3)
            print(f"Total size:     {total_gb:.2f} GB")
            print(f"File count:     {len(files)}")

            print()
            print("Downloaded files:")
            for f in sorted(files):
                if f.is_file():
                    size_mb = f.stat().st_size / (1024*1024)
                    if size_mb >= 1024:
                        print(f"  [OK] {f.name:<50} {size_mb/1024:>8.2f} GB")
                    else:
                        print(f"  [OK] {f.name:<50} {size_mb:>8.2f} MB")
        else:
            print()
            print(" WARNING: No files were downloaded!")
            print()
            print(" Possible solutions:")
            print("  1. Check your internet connection")
            print("  2. Try a different mirror (e.g., https://huggingface.co directly)")
            print("  3. Use VPN/proxy if you're in a restricted network region")
            sys.exit(1)

    print("=" * 70)

    if fail_count == 0 and success_count > 0:
        print(" SUCCESS! All files downloaded successfully.")
        return 0
    elif success_count > 0:
        print(" PARTIAL SUCCESS. Some files may be missing.")
        return 1
    else:
        print(" FAILED. No files could be downloaded.")
        return 1

if __name__ == "__main__":
    sys.exit(main())
