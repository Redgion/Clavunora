# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
param(
    [Parameter(Mandatory=$true)][string]$File,
    [Parameter(Mandatory=$true)][string]$CertificateThumbprint,
    [Parameter(Mandatory=$true)][string]$TimestampUrl
)
$ErrorActionPreference = 'Stop'
# The private key remains in the owner's Windows certificate store.
signtool sign /sha1 $CertificateThumbprint /fd SHA256 /tr $TimestampUrl /td SHA256 $File
if ($LASTEXITCODE -ne 0) { throw 'Code signing failed' }
signtool verify /pa /v $File
if ($LASTEXITCODE -ne 0) { throw 'Signature verification failed' }
Write-Output 'Signed. Recreate affected packages and checksums before publishing.'
