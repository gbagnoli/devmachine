# Physical installation

Status: media generation is implemented. Beelink S13 installation, physical TPM,
MOK enrollment, and LAN acceptance remain unverified. VM commands continue to
use `clamps.bu` with `/dev/vda`; never compile that template directly for hardware.
See [storage policy](../design/storage.md) and [remaining encryption checks](../plan/TPM-ENCRYPTED-ROOT.md).

## Prepare

1. Boot a verified Fedora CoreOS **x86_64 live ISO** on the server to inspect it.
   Attach Ethernet, a keyboard and a monitor. Identify the stock SSD:

   ```sh
   lsblk -o NAME,SIZE,MODEL,SERIAL,TYPE,MOUNTPOINTS
   ls -l /dev/disk/by-id/
   ls /sys/class/tpm
   mokutil --sb-state
   ```

   Record the SSD's whole-disk `/dev/disk/by-id/...` path, not a `-partN` link.
   The path belongs to the server; it need not exist on the workstation.
   Keep rupik and its production SSD running until the scheduled cutover.

2. Prepare private operator inputs outside the checkout: an Ed25519 public key
   and a **fresh bootstrap password hash** (yescrypt or SHA-256/SHA-512 crypt).
   Use an existing password-hashing tool that prompts, keeping the plaintext
   password off command lines and logs. Save the hash in a mode-0600 file.
   Run the following on the **workstation**, using existing OpenSSL. The
   directory follows XDG and defaults to `~/.local/share/skillet/install-inputs`:

   ```sh
   install_inputs="${XDG_DATA_HOME:-$HOME/.local/share}/skillet/install-inputs"
   mkdir -p "$install_inputs"
   chmod 700 "$install_inputs"
   (
     umask 077
     openssl passwd -6 > "$install_inputs/bootstrap.hash"
   )
   ```

   OpenSSL prompts for the initial `giacomo` account password. Do not put the
   plaintext password on the command line or share it or its hash in logs/chat.
   Know that password: access finalization later requires it and forces a
   password change on first login. Media generation
   replaces the inherited public keys and committed bootstrap hash; it does not
   generate or store an operator's SSH private key.

3. On the workstation, download the public ublue DER signing certificate
   referenced by the [ublue Secure Boot instructions](https://github.com/ublue-os/bluefin#secure-boot):

   ```sh
   curl --fail --location \
     https://raw.githubusercontent.com/ublue-os/akmods/main/certs/public_key.der \
     --output "$install_inputs/ublue.der"
   openssl x509 -inform DER -in "$install_inputs/ublue.der" \
     -noout -subject -fingerprint -sha256
   ```

   The certificate subject identifies `Universal Blue` and `ublue kernel`.
   This public certificate is included in the prepared USB for MOK enrollment.
   TPM requires UEFI, TPM 2 and Secure Boot with that MOK enrolled. Supply the
   certificate for unencrypted installation too if Secure Boot is enabled.
   Verify the live ISO's signature using Fedora's published verification flow.
   `--iso` is an already verified local ISO; Skillet does not verify its signature.

4. From `ublue/skillet` on the workstation, in the same shell as steps 2–3,
   prepare a **new private output directory**:

   ```sh
   export PATH=/opt/x86_64-linux-musl-cross/bin:$PATH
   cargo run --release -p skillet -- install prepare clamps \
     --disk /dev/disk/by-id/WHOLE_DISK_ID \
     --iso /absolute/path/fedora-coreos-live.x86_64.iso \
     --output /absolute/private/path/clamps-install \
     --ssh-public-key /absolute/path/operator_ed25519.pub \
     --password-hash-file "$install_inputs/bootstrap.hash" \
     --root-profile tpm \
     --mok-certificate "$install_inputs/ublue.der"
   ```

   Use `--root-profile unencrypted` for a machine without TPM; this is the
   default. A requested TPM profile never silently falls back. Preparing TPM
   media does not prove that the destination hardware supports it.
   The existing Podman/Butane/CoreOS installer containers and musl toolchain
   are required; no workstation packages are installed.

   The bundle includes `install.iso`, destination `clamps.ign`, live `live.ign`,
   and staged source/binaries. It contains the private password hash: keep the
   whole bundle outside Git, protect the USB, and delete it after acceptance.
   Existing output directories are refused. No workstation block device is opened.
   Failed tooling leaves a private diagnostic file named in the error; treat it
   as sensitive and remove it after inspection.

## Write and install

5. Identify the **USB device on the workstation** with `lsblk`; it is separate
   from the destination SSD declared above. Write the generated `install.iso`
   using an existing image writer. If using `dd`, check the whole USB identity
   before running this destructive command:

   ```sh
   sudo dd if=/absolute/private/path/clamps-install/install.iso \
     of=/dev/disk/by-id/WHOLE_USB_ID bs=4M conv=fsync status=progress
   ```

6. Boot that USB in UEFI mode. It starts a live environment and **does not
   automatically install**. Find its temporary DHCP address with `ip -brief address`.
   The supplied operator key also grants SSH access to the live `core` account.
   Verify its SSH host-key fingerprint at the console before accepting it:

   ```sh
   sudo ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub
   ```

   From the workstation, transfer the prepared destination Ignition:

   ```sh
   scp -i /absolute/path/operator_ed25519 \
     /absolute/private/path/clamps-install/clamps.ign core@LIVE_ADDRESS:clamps.ign
   ```

   On the server's live console or an interactive SSH session, run:

   ```sh
   sudo /var/usrlocal/bin/skillet-install /home/core/clamps.ign
   ```

   The helper copies the supplied file into root-only live tmpfs and checks its
   SHA-256 against the digest embedded in this USB. A payload from another bundle
   is refused. Destination Ignition remains in the private bundle and travels
   over SSH. See [the media design](../design/storage.md#physical-installation-media).
   An offline alternative is a separate data USB: mount it and pass the Ignition
   path to the same helper.

   The helper checks the prepared disk exists and is a whole disk. TPM media
   additionally checks UEFI, TPM 2 and Secure Boot. With Secure Boot enabled,
   it requires the supplied ublue certificate to be enrolled. If needed:

   ```sh
   sudo mokutil --import /etc/skillet-install/ublue.der
   ```

   `mokutil` asks you to choose a temporary enrollment password, separate
   from the account password. Enter that same password in the firmware MOK
   enrollment screen after reboot. Then boot this USB again.
   The helper requires the explicit `is already enrolled` result. Supported
   live images may return either 0 or 1 for that result; pending enrollment,
   blocked certificates, unknown output and tool errors are refused.
   If an older prepared USB rejects this confirmed result, copy the current
   `ublue/butane/bin/install-hardware` from the workstation to
   `/home/core/skillet-install.fixed` over SSH. On the live server, update only
   the helper and retry:

   ```sh
   sudo install -m 0700 /home/core/skillet-install.fixed /var/usrlocal/bin/skillet-install
   sudo /var/usrlocal/bin/skillet-install /home/core/clamps.ign
   ```

   Keep the destination Ignition, checksum and disk/profile files from the
   original bundle unchanged. The helper update lasts only for this live boot;
   newly prepared ISOs include the corrected check.
   Retry the helper after the final Secure Boot/MOK policy is established.
   It displays the disk model/serial/size and requires typing its complete
   prepared identifier before running `coreos-installer`. Installation erases
   that disk. A hardware mismatch stops installation; regenerate the bundle
   instead of editing its target ad hoc.

7. Remove the USB and reboot. Allow FCOS-to-uCore signed bootstrap and its
   intermediate reboots to complete. Connect using the supplied SSH key;
   verify the host key independently. Check signed boot, Skillet base apply,
   Btrfs `/var/lib/data`, DNS, user environment and enforcing SELinux.

## Provision and cut over

8. Unlock KeePassXC and use the existing production `secrets deliver` commands
   in [Skillet's README](../skillet/README.md). Check the production secret audit
   first. Service delivery may activate the full host configuration: keep the
   new controller isolated from production devices until the planned migration.

9. For encrypted root, run the documented `secrets root-recovery-key clamps`
   command **before finalizing sudo**. Back up the resulting vault, including
   its verified recovery key and `root-luks.header` attachment, off the server.
   Test cold boot, offline unlock and an approved OS update on the physical box.

10. Follow the [migration plan](../plan/CLAMPS-MIGRATION.md): test Pi-hole from
    LAN clients; production HTTPS and outside-tailnet denial; transfer service
    data/identities; stop the old UniFi controller before restoring/enabling the
    new one; switch DNS/DDNS with one writer and retain rupik for rollback.
    Establish off-machine data backups; local snapshots are not a disk backup.

11. Finish with `sudo /var/usrlocal/bin/skillet access finalize --user giacomo`.
    Keep the current session open, change the password in a second interactive
    login and confirm sudo asks for it. Automated credential delivery currently
    assumes temporary passwordless sudo, so complete it before this step.

PXE is an alternative transport for the same destination Ignition, but this
command currently produces USB ISO media only. Firmware PXE support and the
DHCP/boot-file server setup remain separate work.
