FROM registry.fedoraproject.org/fedora:44
RUN dnf install -y systemd && dnf clean all
