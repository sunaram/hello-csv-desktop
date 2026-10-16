# Privacy Policy

**Hello CSV** - published by Patir Softwares
Effective date: 16 October 2026

## Summary

Hello CSV does not collect, store, transmit, sell or share any personal information. It has no accounts, no analytics,
no advertising and no telemetry.

## What the app does with your files

The app reads the CSV / TSV files that you open, on your device, in order to display and edit them. Their content is never
uploaded or sent anywhere by the app. When you press **Save**, the app writes your changes back to the same file on your
device (through a temporary file next to it, which is renamed over the original and removed if anything fails). Searching,
sorting and every other operation happen entirely on your device. The app does not run scripts or load web content found
inside your files: cell values are only ever shown as text.

## What the app stores on your device

The app saves a small settings file in your user profile (`%APPDATA%\hellocsv` on Windows, `~/.config/hellocsv` on Linux,
`~/Library/Application Support/hellocsv` on macOS) containing:

- your theme choice (system, light or dark),
- your zoom level,
- whether the update check is enabled (downloadable builds only), and
- the file paths of the last ten files you opened (the "Open Recent" list).

The operating system's web view component also keeps its normal cache in the `webview` folder next to that settings file.
This data stays on your device. You can delete it at any time by removing the folder (uninstalling the app also removes it
on Windows when installed from the Microsoft Store).

## Network access

The app itself makes no network connections and sends no data to the publisher or any third party.

The one exception is the **update check in the downloadable builds** (the standalone `.exe`, Linux and macOS packages from
GitHub Releases). At most once a day it asks the public GitHub API for the latest release version
(`https://api.github.com/repos/sunaram/hello-csv-desktop/releases/latest`) and shows a notice if a newer one exists. No
information about you, your files or your device is sent beyond what any web request includes (such as your IP address, which
GitHub can see). You can turn it off in **Help > Check for updates at startup**, or run it on demand with **Help > Check for
Updates**. The version installed from the **Microsoft Store never makes this request**: the Store delivers updates.

## Children

The app is a general-purpose file viewer and editor. It does not collect personal information from anyone, including children.

## Open source

Hello CSV is open source under the MIT license. You can inspect exactly what it does at
<https://github.com/sunaram/hello-csv-desktop>.

## Changes to this policy

If this policy changes, the updated version will be published at
<https://github.com/sunaram/hello-csv-desktop/blob/main/PRIVACY.md> with a new effective date.

## Contact

Questions about privacy can be raised by opening an issue at <https://github.com/sunaram/hello-csv-desktop/issues>.
