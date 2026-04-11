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

# Mock podman if it doesn't exist
if [ ! -x /usr/bin/podman ]; then
    echo "Mocking podman..."
    cat <<EOF > /usr/bin/podman
#!/bin/sh
case "\$*" in
    "secret inspect"*)
        exit 1 # Secret doesn't exist
        ;;
    "secret create"*)
        exit 0 # Successfully created
        ;;
    *)
        echo "Mock podman: \$@"
        exit 0
        ;;
esac
EOF
    chmod +x /usr/bin/podman
fi

# Execute the passed command (skillet apply)
exec "$@"
