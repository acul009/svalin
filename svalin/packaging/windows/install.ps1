$banner = @"
  _____    __      __               _         _____     _   _
 / ____|   \ \    / /     /\       | |       |_   _|   | \ | |
| (___      \ \  / /     /  \      | |         | |     |  \| |
 \___ \      \ \/ /     / /\ \     | |         | |     |     |
 ____) |      \  /     / ____ \    | |____    _| |_    | |\  |
|_____/        \/     /_/    \_\   |______|  |_____|   |_| \_|

"@

$default_install_path = "C:\Program Files\Svalin"
$exe_file = "svalin"
$exe_path = "$default_install_path\$exe_file"
$agent_service_name = "svalin-agent"
$uninstall_key = "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\SvalinAgent"

function Get-UserChoice {
    param (
        [string]$Prompt,
        [string[]]$ValidChoices
    )

    do {
        Write-Host ""
        Write-Host $Prompt
        Write-Host ""
        $choice = Read-Host "Select an Option"
        if ($ValidChoices -notcontains $choice) {
            Write-Host "Invalid selection, please try again."
        }
    } until ($ValidChoices -contains $choice)
    Write-Host ""
    return $choice
}

Write-Host $banner

if (-not ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Host "Sorry, but you need to run this script as an Administrator to install Svalin."
    Write-Host "Aborting setup, Goodbye!"
    Write-Host ""
    Exit 1
}

$prompt = @"
Welcome to the Svalin setup script. What do you want to do?

[1] Install Svalin agent
[2] Uninstall Svalin agent
[0] Exit
"@

$choice = Get-UserChoice -Prompt $prompt -ValidChoices @("1", "2", "0")

switch ($choice) {
    "0" { 
        Write-Host ""
        Write-Host "Goodbye!"
        Write-Host ""
        exit 1
    }
    "1" {


        $prompt = @"
Which branch would you like to install?

[1] Alpha
"@

        $branch_number = Get-UserChoice -Prompt $prompt -ValidChoices @("1")

        switch ($branch_number) {
            "1" {
                $branch = "-alpha"
            }
        }

        $apiUrl = "https://api.github.com/repos/acul009/svalin/releases"
        $headers = @{ "User-Agent" = "PowerShell" }

        try {
            $releases = Invoke-RestMethod -Uri $apiUrl -Headers $headers
        }
        catch {
            Write-Host "Failed to retrieve releases: $_"
            exit 1
        }

        $filtered = $releases | Where-Object { $_.tag_name -match "$branch$" }
        if (-not $filtered) {
            Write-Host "No releases found for branch '$branch'."
            exit 1
        }

        $latest = $filtered | Sort-Object { [datetime]$_.published_at } -Descending | Select-Object -First 1

        $asset = $latest.assets | Where-Object { $_.name -eq $exe_file }
        if (-not $asset) {
            Write-Host "Asset '$exe_file' not found in release '$($latest.tag_name)'."
            exit 1
        }

        $prompt = @"
This script will install svalin under "$default_install_path".

Do you want to continue? (y/n)
"@

        $choice = Get-UserChoice -Prompt $prompt -ValidChoices @("y", "n")

        if ($choice -eq "n") {
            Write-Host ""
            Write-Host "Aborting setup, Goodbye!"
            Write-Host ""
            exit 1
        }

        if (Test-Path "$default_install_path") {
            $prompt = @"
It seems like Svalin is already installed under "$default_install_path".

Do you want to overwrite it? (y/n)
"@

            $choice = Get-UserChoice -Prompt $prompt -ValidChoices @("y", "n")

            if ($choice -eq "n") {
                Write-Host ""
                Write-Host "Aborting setup, Goodbye!"
                Write-Host ""
                exit 1
            }

            Remove-Item -Path "$exe_path" -Force -ErrorAction SilentlyContinue

            if (Test-Path "$exe_path") {
                Write-Host ""
                Write-Host "Failed to remove old executable."
                Write-Host "Aborting setup, Goodbye!"
                Write-Host ""
                exit 1
            }
        }

        Write-Host "Creating install folder..."

        New-Item -Path $default_install_path -ItemType Directory -Force | Out-Null

        if (-not (Test-Path "$default_install_path")) {
            Write-Host ""
            Write-Host "Failed to create install folder."
            Write-Host "Aborting setup, Goodbye!"
            Write-Host ""
            exit 1
        }

        Write-Host "Downloading '$exe_file' from release '$($latest.tag_name)'..."
        try {
            Invoke-WebRequest -Uri $asset.browser_download_url -OutFile "$exe_path" -Headers $headers
            Write-Host "Download complete. File saved to '$exe_path'."
        }
        catch {
            Write-Host "Download failed: $_"
            exit 1
        }

        $start_type = "Disabled"

        $current_service = Get-Service -Name $agent_service_name -ErrorAction SilentlyContinue

        if ($current_service) {
            $start_type = $current_service.StartType
        }


        if (Get-Command Remove-Service -ErrorAction SilentlyContinue) {
            Remove-Service -Name $agent_service_name -Force
        }
        else {
            sc.exe delete $agent_service_name | Out-Null
        }


        New-Service -Name $agent_service_name -DisplayName "Svalin Agent" -BinaryPathName "`"$exe_path`"" -StartupType $start_type | Out-Null
        Write-Host "Service '$agent_service_name' created successfully."

        $folder_size_bytes = (Get-ChildItem -Path $default_install_path -Recurse -File | Measure-Object -Property Length -Sum).Sum
        $folder_size_kb = [math]::Round($folder_size_bytes / 1KB, 2)

        if (Test-Path $uninstall_key) { Remove-Item -Path $uninstall_key -Recurse -Force }
        New-Item -Path $uninstall_key -Force | Out-Null
        New-ItemProperty -Path $uninstall_key -Name "DisplayName" -Value "Svalin" -PropertyType String -Force | Out-Null
        New-ItemProperty -Path $uninstall_key -Name "DisplayVersion" -Value "$($latest.tag_name)" -PropertyType String -Force | Out-Null
        New-ItemProperty -Path $uninstall_key -Name "Publisher" -Value "acul009 <luca@it-rahn.de>" -PropertyType String -Force | Out-Null
        New-ItemProperty -Path $uninstall_key -Name "InstallLocation" -Value $default_install_path -PropertyType String -Force | Out-Null
        New-ItemProperty -Path $uninstall_key -Name "EstimatedSize" -Value $folder_size_kb -PropertyType DWord -Force | Out-Null
        # New-ItemProperty -Path $uninstall_key -Name "UninstallString" -Value "powershell.exe -File `"$default_install_path\uninstall.ps1`"" -PropertyType String -Force | Out-Null
        # New-ItemProperty -Path $uninstall_key -Name "DisplayIcon" -Value "$default_install_path\svalin.exe" -PropertyType String -Force | Out-Null

        Write-Host ""
        Write-Host ""
        Write-Host "Svalin agent has been installed successfully!"
        Write-Host ""
        Write-Host ""
    }
    "2" { 
        Write-Host "Uninstalling Svalin agent..."
    }
}