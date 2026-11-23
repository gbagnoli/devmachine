ARG FEDORA_VERSION=44
FROM registry.fedoraproject.org/fedora:${FEDORA_VERSION}
RUN dnf -y install clevis clevis-luks clevis-pin-tpm2 tpm2-tools cryptsetup swtpm util-linux && dnf clean all
