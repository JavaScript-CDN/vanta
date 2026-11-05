#!/bin/bash
# Standard public runners have limited free disk; remove preinstalled SDKs we never use
# and add swap so the ThinLTO link step does not run out of memory.
set -euxo pipefail
df -h /
sudo rm -rf /usr/local/lib/android /usr/local/.ghcup /opt/ghc /usr/lib/jvm \
    /usr/lib/google-cloud-sdk /usr/share/dotnet /usr/lib/dotnet /usr/share/swift \
    /opt/hostedtoolcache/CodeQL /usr/local/share/powershell /usr/local/share/chromium \
    /usr/local/lib/node_modules || true
sudo docker image prune -a -f || true
sudo swapoff -a || true
sudo rm -f /mnt/swapfile /swapfile || true
sudo fallocate -l 10G /mnt/swapfile && sudo chmod 600 /mnt/swapfile \
    && sudo mkswap /mnt/swapfile && sudo swapon /mnt/swapfile || true
free -h; df -h / /mnt
