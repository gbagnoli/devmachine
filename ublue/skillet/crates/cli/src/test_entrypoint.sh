#!/bin/sh
set -e

# Ensure /etc/sysctl.d exists (often missing in minimal containers)
mkdir -p /etc/sysctl.d

# Mock systemctl if it doesn't exist
if [ ! -x /usr/bin/systemctl ]; then
    echo "Mocking systemctl..."
    cat <<EOF > /usr/bin/systemctl
#!/bin/sh
echo "Mock systemctl: \$@"
exit 0
EOF
    chmod +x /usr/bin/systemctl
fi

# Execute the passed command (skillet apply)
exec "$@"
