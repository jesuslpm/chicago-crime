<#
.SYNOPSIS
    Identity of the reference extract, dot-sourced by the acceptance scripts.

.DESCRIPTION
    The exact figures of the PRDs hold only for the reference extract (README, "Get the dataset"). The
    acceptance scripts compare this fingerprint with the source SHA-256 that the generator records in
    the manifest: on a match they require the exact figures, otherwise they count the expected figures
    on the CSV itself (PRD aceptacion-cualquier-csv, decisions A1 and A2).
#>

$ReferenceSha256 = '5cb5fa41c930ece2e91f683d894c815d46333d2a5372b8df5ae40b6e8a6ae35d'

function Test-ReferenceExtract([string]$Sha256) { $Sha256 -eq $ReferenceSha256 }

# Whether a manifest's source fingerprint is a SHA-256 at all: a malformed one would silently make the
# reference extract look like another CSV, so the scripts fail on it.
function Test-Sha256([string]$Value) { $Value -match '^[0-9a-fA-F]{64}$' }

# The report header line that says which CSV was checked.
function Format-CsvMode([string]$Sha256) {
    if (Test-ReferenceExtract $Sha256) {
        'CSV: the reference extract (SHA-256 {0}); the exact figures of the PRDs are required.' -f $Sha256
    }
    else {
        'CSV: not the reference extract (SHA-256 {0}); the expected figures are counted on this CSV.' -f $Sha256
    }
}
