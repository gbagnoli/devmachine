FROM registry.fedoraproject.org/fedora:44
RUN dnf -y install clevis clevis-luks clevis-pin-tpm2 tpm2-tools cryptsetup swtpm util-linux && dnf clean all
