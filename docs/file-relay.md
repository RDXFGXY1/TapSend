# File Relay

File Relay delivers a file between two Windows computers when the destination is asleep or TapSend is closed there. A third computer needs to stay on and provide a shared folder both endpoints can write to. That folder holds encrypted file contents until the destination returns.

## Set up the route

1. Confirm the sending PC and destination PC can open the same writable network share in File Explorer, for example `\\relay-pc\share`.
2. On the **destination PC**, open TapSend. Press **:** and run `relay create \\relay-pc\share` with the real share path. TapSend copies a relay pass to the clipboard.
3. Move the pass privately to the **sending PC**. Copy it there and press **Shift+J** in TapSend.

The pass gives access to the encrypted lane. Share it only with the sending endpoint you trust.

## Send and confirm a file

1. On the sending PC, press **F** and focus a file. Press **:** and run `relay send`.
2. Run `relay status` to see the latest delivery. **Holding** means the file is staged on the relay for the destination.
3. Start TapSend on the destination PC. It collects the file into the **Relay** folder inside its configured inbox. The sender's status changes to **SAVED** once the destination confirms.

Keep the shared folder available until the destination collects the file. File Relay currently uses Windows endpoints and a network share. For delivery from Windows to an offline paired Android phone, use [Send Later](transfers.md#send-later).
