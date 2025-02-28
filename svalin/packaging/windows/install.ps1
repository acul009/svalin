$banner = @"
  _____    __      __               _        _____     _   _
 / ____|   \ \    / /     /\       | |      |_   _|   | \ | |
| (___      \ \  / /     /  \      | |        | |     |  \| |
 \___ \      \ \/ /     / /\ \     | |        | |     | . ` |
 ____) |      \  /     / ____ \    | |____   _| |_    | |\  |
|_____/        \/     /_/    \_\   |______|  |_____|  |_| \_|
"@

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
        exit
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
            return
        }

        $filtered = $releases | Where-Object { $_.tag_name -match "$branch$" }
        if (-not $filtered) {
            Write-Host "No releases found for branch '$branch'."
            return
        }

        $latest = $filtered | Sort-Object { [datetime]$_.published_at } -Descending | Select-Object -First 1

        $asset_name = "svalin.exe"

        $asset = $latest.assets | Where-Object { $_.name -eq $asset_name }
        if (-not $asset) {
            Write-Host "Asset '$asset_name' not found in release '$($latest.tag_name)'."
            return
        }

    }
    "2" { 
        Write-Host "Uninstalling Svalin agent..."
    }
}