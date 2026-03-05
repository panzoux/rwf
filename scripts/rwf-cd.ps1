# Wrapper script for rwf that changes directory on exit (PowerShell version)
# 
# Usage:
#   Add this function to your PowerShell profile:
#   
#   1. Open your profile:
#      notepad $PROFILE
#   
#   2. Add the following lines:
#      . C:\path\to\rwf-cd.ps1
#      Set-Alias rwf Invoke-RwfCd
#   
#   3. Reload your profile:
#      . $PROFILE

function Invoke-RwfCd {
    [CmdletBinding()]
    param(
        [Parameter(ValueFromRemainingArguments=$true)]
        [string[]]$Arguments
    )
    
    # Run rwf with -cwd flag and capture the output directory
    $outputDir = & rwf --cwd @Arguments 2>&1 | Select-Object -Last 1
    $exitCode = $LASTEXITCODE
    
    # If rwf exited successfully and output a directory, change to it
    if ($exitCode -eq 0 -and $outputDir -and (Test-Path -Path $outputDir -PathType Container)) {
        Set-Location -Path $outputDir
    }
    
    # Return the exit code
    return $exitCode
}

# Export the function
Export-ModuleMember -Function Invoke-RwfCd
