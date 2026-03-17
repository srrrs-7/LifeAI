#!/bin/bash
set -e

echo "Starting Dev Container setup..."

cargo build

make hooks
sudo make init-firewall

if [ ! -f ".devcontainer/setup.personal.sh" ]; then
  cat << 'PERSONAL' > .devcontainer/setup.personal.sh
#!/bin/bash
set -e

# Your personal setup steps here
PERSONAL
  chmod +x .devcontainer/setup.personal.sh
fi

bash .devcontainer/setup.personal.sh

if [ "${ENABLE_FIREWALL:-false}" = "true" ]; then
  echo "Setting up firewall..."
  if [ -f ".devcontainer/init-firewall.sh" ]; then
    sudo bash .devcontainer/init-firewall.sh
  else
    echo "Firewall script not found, skipping..."
  fi
fi

echo "Dev Container setup completed."
