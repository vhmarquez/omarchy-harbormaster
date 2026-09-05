# Public distro tools only. Build context is tools/, never the source or .tools.
# Input digests are qualified in platform-lock.json and docs/DEPENDENCIES.md.
FROM docker.io/library/archlinux@sha256:82b1b08faae9d61e3e7e13d562f4d09114d939105b0d59ff34140f3bd418593a

SHELL ["/bin/bash", "-euo", "pipefail", "-c"]
COPY platform-lock.json /opt/harbormaster-platform-lock.json

# Pin repository bytes BEFORE resolving/installing any package. Never -Sy/-Syu:
# -Suu uses only these checked databases, upgrading/downgrading the base to them.
# DatabaseOptional does not waive the mandatory package signatures or SHA pins.
RUN printf '%s\n' \
      '[options]' 'Architecture = x86_64' \
      'SigLevel = Required DatabaseOptional' \
      'LocalFileSigLevel = Required' 'RemoteFileSigLevel = Required' \
      'XferCommand = /usr/bin/curl --fail --location --proto =https --proto-redir =https --connect-timeout 15 --max-time 180 --retry 2 --output %o %u' \
      '[core]' 'Server = https://archive.archlinux.org/repos/2026/09/05/$repo/os/$arch' \
      '[extra]' 'Server = https://archive.archlinux.org/repos/2026/09/05/$repo/os/$arch' \
      > /etc/pacman.conf \
    && mkdir -p /var/lib/pacman/sync \
    && for repo in core extra; do \
      curl --fail --location --proto '=https' --proto-redir '=https' \
        --connect-timeout 15 --max-time 180 --retry 2 \
        "https://archive.archlinux.org/repos/2026/09/05/$repo/os/x86_64/$repo.db" \
        --output "/var/lib/pacman/sync/$repo.db"; \
    done \
    && printf '%s\n' \
      'f1893be2c432e2e0cbec967bde8930efdfbd96ed243078c6e2474c983569d782  /var/lib/pacman/sync/core.db' \
      'd0d5e9b898dea4e646e4f12a351e2a31b49c6fca5f1c682383a5e7f1dfc9f23d  /var/lib/pacman/sync/extra.db' \
      | sha256sum --check --strict \
    && pacman -Suu --noconfirm --needed archlinux-keyring bubblewrap gcc git python qt6-declarative nodejs \
    && pacman -Scc --noconfirm

# Keep the snapshot selection: local Qt's package release may differ, while
# the same Qt 6.11.2 runtime/tool release is required by both verifier backends.
RUN test "$(pacman -Q qt6-base)" = 'qt6-base 6.11.2-3' \
    && test "$(pacman -Q qt6-declarative)" = 'qt6-declarative 6.11.2-1' \
    && test "$(pacman -Q python)" = 'python 3.14.7-1' \
    && test "$(pacman -Q nodejs)" = 'nodejs 26.8.1-2' \
    && test "$(pacman -Q bubblewrap)" = 'bubblewrap 0.12.0-1' \
    && test -x /usr/bin/bwrap \
    && python3 -c 'import json, subprocess; p=json.load(open("/opt/harbormaster-platform-lock.json")); expected=[n+" "+r["version"] for n,r in p["packages"].items()]; actual=[subprocess.check_output(["pacman","-Q",n],text=True).strip() for n in p["packages"]]; assert actual == expected, (actual, expected)' \
    && test "$(python3 --version)" = 'Python 3.14.7' \
    && test "$(node --version)" = 'v26.8.1' \
    && test "$(/usr/lib/qt6/bin/qmllint --version)" = 'qmllint 6.11.2' \
    && test "$(/usr/lib/qt6/bin/qmlformat --version)" = 'qmlformat 6.11.2' \
    && test -x /usr/lib/qt6/bin/qmltestrunner \
    && pacman -Q > /opt/harbormaster-image-packages.txt

# No user home, credentials, source, Rust tool downloads or Docker socket baked
# in. verify.py supplies /work and /tools read-only and private writable /state,
# clears the environment and enforces the runtime namespace/resource policy.
USER 65532:65532
WORKDIR /work
ENTRYPOINT []
CMD ["/usr/bin/false"]
