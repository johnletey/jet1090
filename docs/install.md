# Installation instructions

The latest release and installation instructions are published [on GitHub Releases](https://github.com/xoolive/jet1090/releases/).

## Install prebuilt binaries

=== "Shell script"

    ```sh
    curl --proto '=https' --tlsv1.2 -LsSf https://github.com/xoolive/jet1090/releases/latest/download/jet1090-installer.sh | sh
    ```

    Update to next release with:

    ```sh
    jet1090-update
    ```

=== "Arch Linux"

    ```sh
    pacman -S jet1090-bin
    ```

=== "Homebrew (Mac OS)"

    ```sh
    brew install xoolive/homebrew/jet1090
    ```

    Update to next release with:

    ```sh
    brew upgrade
    ```

=== "Powershell (Windows)"

    ```sh
    powershell -ExecutionPolicy ByPass -c "irm https://github.com/xoolive/jet1090/releases/latest/download/jet1090-installer.ps1 | iex"
    ```

    - Update to next release with:
      ```sh
      jet1090-update
      ```

    - You will also need [Zadig](https://zadig.akeo.ie/) to install the drivers for your SDR (admin rights necessary).

## Dependencies

The prebuilt binaries are compiled with default features activated.

If you compile the executable with the `soapy` feature, you will need extra dependencies.

## Build from source

=== "cargo"

    Build with all features:
    ```sh
    cargo install --all-features jet1090
    ```

    Build with no default features:
    ```sh
    cargo install --no-default-features jet1090
    ```

    You will need:

    - the SoapySDR dependencies to compile with the `soapy` feature.

    === "Ubuntu"

        ```sh
        sudo apt install libsoapysdr-dev  # plus support for other devices
        ```

    === "Arch Linux"

        ```sh
        pacman -S soapysdr
        ```

    === "Homebrew"

        ```sh
        brew install soapysdr  # plus support for other devices
        ```

    === "Windows"

        - Install [PothosSDR](https://downloads.myriadrf.org/builds/PothosSDR/PothosSDR-2021.07.25-vc16-x64.exe). If you don't have admin rights, you may unzip the archive and add the `bin/` folder to your `PATH` variable.

    - a protobuf compiler to compile with the `sero` feature.


    === "Ubuntu"

        ```sh
        sudo apt install protobuf-compiler
        ```

    === "Arch Linux"

        ```sh
        pacman -S protobuf
        ```

    === "Homebrew"

        ```sh
        brew install protobuf
        ```

    === "Windows"

        - Install [Protobuf compiler](https://github.com/protocolbuffers/protobuf/releases/download/v28.3/protoc-28.3-win64.zip)

=== "nix"

    Nix takes care of its own dependencies. The script has been tested for Linux and MacOS.

    ```sh
    git clone https://github.com/xoolive/jet1090
    nix profile install
    ```

## Shell completion

=== "Bash"

    Add the following to the end of your `~/.bashrc`:

    ```sh
    eval "$(jet1090 --completion bash)"
    ```

=== "Zsh"

    Add the following to the end of your `~/.zshrc`:

    ```sh
    eval "$(jet1090 --completion zsh)"
    ```

=== "fish"

    Add the following to the end of `~/.config/fish/config.fish`:

    ```fish
    jet1090 --completion fish | source
    ```

=== "Powershell"

    Add the following to the end of `Microsoft.PowerShell_profile.ps1`. You can check the location of this file by querying the `$PROFILE` variable in PowerShell. Typically the path is` ~\Documents\PowerShell\Microsoft.PowerShell_profile.ps1` or `~/.config/powershell/Microsoft.PowerShell_profile.ps1` on -Nix.

    ```powershell
    (& jet1090 --completion powershell) | Out-String | Invoke-Expression
    ```

=== "Nushell"

    Add the following to the end of your Nushell env file (find it by running `$nu.env-path`):

    ```sh
    mkdir -p ~/.config/jet1090
    jet1090 --completion nushell | save -f ~/.config/jet1090/completions.nu
    ```

    then, add the following to the end of your Nushell configuration (find it by running `$nu.config-path`):

    ```sh
    use ~/.config/jet1090/completions.nu *
    ```

=== "Elvish"

    Add the following to the end of `~/.elvish/rc.elv`:

    ```sh
    eval (jet1090 --completion elvish | slurp)
    ```
