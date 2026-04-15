#!/bin/bash
COMMAND="$1"
shift # Shift past the command

USE_ROOT=false
AGENT_NAME="default-agent"

# Parse remaining arguments to catch --root and the agent name
for arg in "$@"; do
    if [ "$arg" == "--root" ]; then
        USE_ROOT=true
    else
        AGENT_NAME="$arg"
    fi
done

PROJECT_DIR="$(pwd)"
SCRIPT_NAME="$(basename "$0")"
# Named volume for the home dir — isolated from your project files on the host.
# Podman manages it under ~/.local/share/containers; nothing leaks into PROJECT_DIR.
HOME_VOLUME="${AGENT_NAME}-home"

case "$COMMAND" in
    create)
        echo "🏗️  1/3 Creating agent container: '$AGENT_NAME' using pure Alpine..."
        # Named volume keeps ~/.config, ~/.npm, ~/.local etc. off your project folder
        podman volume create "$HOME_VOLUME" > /dev/null

        PODMAN_SOCK="/run/user/$(id -u)/podman/podman.sock"

        podman create \
            -it \
            --name "$AGENT_NAME" \
            --userns=keep-id \
            -v "$HOME_VOLUME:/home/dev" \
            -v "$PROJECT_DIR:/workspace:Z" \
            -v "$PODMAN_SOCK:/run/podman/podman.sock" \
            -e HOME="/home/dev" \
            -e SHELL="/bin/bash" \
            -e CONTAINER_HOST="unix:///run/podman/podman.sock" \
            -w /workspace \
            alpine:latest tail -f /dev/null > /dev/null

        echo "⚙️  2/3 Starting container temporarily to install tools..."
        podman start "$AGENT_NAME" > /dev/null

        echo "🔧 Installing system packages, PHP, Go, Node, and configuring env..."

        # Phase 1: Root installs system packages only (no npm global installs)
        podman exec -u root "$AGENT_NAME" /bin/sh -c "
            set -e
            apk update && apk add --no-cache \
                bash curl git unzip sqlite nano vim tzdata \
                libxml2-dev oniguruma-dev icu-data \
                nodejs npm python3 py3-pip \
                libc6-compat gcompat build-base \
                go php composer podman \
                php-session php-iconv php-pdo php-pdo_sqlite php-pdo_mysql php-pdo_pgsql \
                php-mbstring php-tokenizer php-ctype php-openssl php-curl php-dom \
                php-fileinfo php-xml php-json php-phar php-xmlwriter php-simplexml \
                php-pcntl php-zip php-bcmath php-intl php-gd

            # Create the dev home dir if the mount isn't present at create-time
            mkdir -p /home/dev
            # Ensure /home/dev is owned by the mapped UID (1000) from the start
            # so that all subsequent user-level writes are clean
            chown -R 1000:1000 /home/dev

            # Write global profile entries (PATH, env vars) for interactive shells
            cat >> /etc/profile <<'EOF'
export SHELL=/bin/bash
export HOME=/home/dev
export GOPATH=\$HOME/go
export PNPM_HOME=\$HOME/.local/share/pnpm
export NPM_CONFIG_CACHE=\$HOME/.npm
export NPM_CONFIG_PREFIX=\$HOME/.npm-global
export BUN_INSTALL=\$HOME/.bun
export CONTAINER_HOST="unix:///run/podman/podman.sock"
export PATH=\"/usr/local/go/bin:\$GOPATH/bin:\$HOME/.npm-global/bin:\$PNPM_HOME:\$BUN_INSTALL/bin:\$PATH\"
EOF
        "

        # Phase 2: Install pnpm/yarn/bun as the dev user so cache/store are user-owned
        podman exec -u 1000 "$AGENT_NAME" /bin/sh -l -c "
            set -e
            export HOME=/home/dev
            export NPM_CONFIG_CACHE=\$HOME/.npm
            export NPM_CONFIG_PREFIX=\$HOME/.npm-global
            export PNPM_HOME=\$HOME/.local/share/pnpm
            export BUN_INSTALL=\$HOME/.bun
            export PATH=\"\$HOME/.npm-global/bin:\$PNPM_HOME:\$BUN_INSTALL/bin:\$PATH\"

            mkdir -p \"\$NPM_CONFIG_CACHE\" \"\$NPM_CONFIG_PREFIX\" \"\$PNPM_HOME\"

            # Install pnpm and yarn under the dev user — no sudo, no root
            npm install -g pnpm yarn

            # Install bun via its installer script (writes to \$BUN_INSTALL)
            curl -fsSL https://bun.sh/install | bash

            echo '✔ pnpm version:' \$(pnpm --version)
            echo '✔ yarn version:'  \$(yarn --version)
            echo '✔ bun version:'   \$(\$BUN_INSTALL/bin/bun --version)
        "

        echo "🛑 3/3 Stopping container to save state..."
        podman stop "$AGENT_NAME" > /dev/null
        echo "✅ Initialization complete! Tools are pre-installed."
        echo "👉 Enter it using: ./$SCRIPT_NAME start $AGENT_NAME"
        ;;

    list)
        echo "📊 Your AI Agents (Alpine based):"
        podman ps -a -f ancestor="alpine:latest"
        ;;

    start)
        if ! podman ps -a --format '{{.Names}}' | grep -Eq "^${AGENT_NAME}\$"; then
            echo "❌ Container '$AGENT_NAME' does not exist. Run './$SCRIPT_NAME create $AGENT_NAME' first."
            exit 1
        fi

        podman start "$AGENT_NAME" > /dev/null

if [ "$USE_ROOT" = true ]; then
        echo "🔌 Attaching to '$AGENT_NAME' as ROOT (Workspace: /workspace)..."
        podman exec -it -u root "$AGENT_NAME" /bin/bash -l
    else
        echo "🔌 Attaching to '$AGENT_NAME' as standard user 1000 (Workspace: /workspace)..."
        podman exec -it -u 1000 "$AGENT_NAME" /bin/bash -l
    fi
        ;;

    stop)
        podman stop "$AGENT_NAME"
        echo "🛑 Stopped '$AGENT_NAME'."
        ;;

    rm)
        podman rm -f "$AGENT_NAME"
        podman volume rm "$HOME_VOLUME" > /dev/null 2>&1 && \
            echo "🗑️  Deleted '$AGENT_NAME' and its home volume." || \
            echo "🗑️  Deleted '$AGENT_NAME' (home volume was already removed)."
        ;;

    *)
        echo "Usage: $SCRIPT_NAME {create|list|start|stop|rm} [agent_name] [--root]"
        echo ""
        echo "Examples:"
        echo "  $SCRIPT_NAME create laravel-dev         -> Creates container & installs full stack"
        echo "  $SCRIPT_NAME start laravel-dev          -> Starts and enters as standard user (1000)"
        echo "  $SCRIPT_NAME start laravel-dev --root   -> Starts and enters as root user"
        echo "  $SCRIPT_NAME list                       -> Shows all your agent containers"
        echo "  $SCRIPT_NAME rm laravel-dev             -> Deletes 'laravel-dev'"
        ;;
esac
