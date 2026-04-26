"""Download GEMMA 4.0 E4B model with progress tracking"""
import os
import sys
import time
from pathlib import Path

try:
    from huggingface_hub import snapshot_download
    from huggingface_hub.utils import ProgressCallback
except ImportError:
    print("ERROR: huggingface_hub not installed")
    print("Please run: pip install huggingface-hub")
    sys.exit(1)

class DownloadProgress(ProgressCallback):
    def on_progress(self, progress):
        if progress.force:
            print(f"\n  Downloading: {progress.filename} ({progress.done}/{progress.total} bytes)")
        elif time.time() - getattr(self, '_last_print', 0) > 2.0:
            pct = (progress.done / progress.total * 100) if progress.total > 0 else 0
            done_mb = progress.done / (1024*1024)
            total_mb = progress.total / (1024*1024)
            print(f"  ... {pct:.1f}% ({done_mb:.1f}MB / {total_mb:.1f}MB)")
            self._last_print = time.time()

repo_id = "google/gemma-4-e4b-it"
target_dir = Path(r"D:\WorkSpace\1文本全结构化知识系统.md\crates\knowledge-api\data\models\google_gemma-4-e4b-it")

print("=" * 70)
print(" GEMMA 4.0 E4B Model Downloader v3 (with progress)")
print("=" * 70)
print(f"\nModel:       {repo_id}")
print(f"Target:      {target_dir}")
print(f"Python:      {sys.version.split()[0]}")
print(f"\nStarting download at {time.strftime('%H:%M:%S')}")
print("-" * 70)
sys.stdout.flush()

start_time = time.time()

try:
    downloaded_path = snapshot_download(
        repo_id=repo_id,
        local_dir=str(target_dir),
        ignore_patterns=["*.bin", "*.pt"],
        tqdm_class=DownloadProgress,
    )

    elapsed = time.time() - start_time

    print("\n" + "=" * 70)
    print(" DOWNLOAD COMPLETE!")
    print("=" * 70)
    print(f"\nLocation:   {downloaded_path}")
    print(f"Time:       {elapsed:.1f} seconds ({elapsed/60:.1f} minutes)")

    print("\n" + "-" * 70)
    print(" Downloaded files:")
    print("-" * 70)

    total_size = 0
    file_list = sorted(Path(downloaded_path).iterdir())

    if not file_list:
        print("\n WARNING: No files found in download directory!")
        print(" The model may have been cached elsewhere by huggingface_hub.")
        sys.exit(1)

    for f in file_list:
        size_mb = f.stat().st_size / (1024 * 1024)
        total_size += f.stat().st_size
        size_gb = size_mb / 1024
        if size_gb >= 1:
            print(f"  [OK] {f.name:<45} {size_gb:>8.2f} GB")
        else:
            print(f"  [OK] {f.name:<45} {size_mb:>8.2f} MB")

    total_gb = total_size / (1024**3)
    print("-" * 70)
    print(f" Total size: {total_gb:.2f} GB")
    print(f" File count: {len(file_list)}")
    print("=" * 70)
    print("\n SUCCESS! Model ready for use.")
    print("=" * 70)

except KeyboardInterrupt:
    print("\n\n\n Download interrupted by user!")
    print(" You can resume by running this script again.")
    sys.exit(130)

except Exception as e:
    print("\n\n" + "=" * 70)
    print(f" DOWNLOAD FAILED!")
    print("=" * 70)
    print(f"\nError type: {type(e).__name__}")
    print(f"Error message: {e}")
    print("\nFull traceback:")
    import traceback
    traceback.print_exc()
    print("\n" + "=" * 70)
    print(" Possible solutions:")
    print("  1. Check your internet connection")
    print("  2. Try using a proxy: set HTTPS_PROXY=http://your-proxy:port")
    print("  3. Use VPN if you're in a region with network restrictions")
    print("  4. Try again later (server may be temporarily unavailable)")
    print("=" * 70)
    sys.exit(1)
