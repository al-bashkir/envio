
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'envio' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'envio'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'envio' {
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('create', 'create', [CompletionResultType]::ParameterValue, 'Create a new profile')
            [CompletionResult]::new('add', 'add', [CompletionResultType]::ParameterValue, 'Add envionment variables to a profile')
            [CompletionResult]::new('load', 'load', [CompletionResultType]::ParameterValue, 'Load all environment variables in a profile for use in your terminal sessions')
            [CompletionResult]::new('unload', 'unload', [CompletionResultType]::ParameterValue, 'Unload a profile')
            [CompletionResult]::new('launch', 'launch', [CompletionResultType]::ParameterValue, 'Run a command with the environment variables from a profile')
            [CompletionResult]::new('remove', 'remove', [CompletionResultType]::ParameterValue, 'Remove a environment variable from a profile')
            [CompletionResult]::new('list', 'list', [CompletionResultType]::ParameterValue, 'List all the environment variables in a profile or all the profiles currenty stored')
            [CompletionResult]::new('update', 'update', [CompletionResultType]::ParameterValue, 'Update environment variables in a profile')
            [CompletionResult]::new('export', 'export', [CompletionResultType]::ParameterValue, 'Export a profile to a file if no file is specified it will be exported to a file named .env')
            [CompletionResult]::new('import', 'import', [CompletionResultType]::ParameterValue, 'Download a profile over the internet and import it into the system or import a locally stored profile into your current envio installation')
            [CompletionResult]::new('sync', 'sync', [CompletionResultType]::ParameterValue, 'Push and pull encrypted profiles to a remote store (S3, Google Drive, or a directory)')
            [CompletionResult]::new('version', 'version', [CompletionResultType]::ParameterValue, 'Print the version')
            [CompletionResult]::new('completion', 'completion', [CompletionResultType]::ParameterValue, 'Generate shell completion scripts')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'envio;create' {
            [CompletionResult]::new('-f', 'f', [CompletionResultType]::ParameterName, 'Read the environment variables from a .env style file')
            [CompletionResult]::new('--file-to-import-envs-from', 'file-to-import-envs-from', [CompletionResultType]::ParameterName, 'Read the environment variables from a .env style file')
            [CompletionResult]::new('-e', 'e', [CompletionResultType]::ParameterName, 'Environment variables to store, as space separated KEY=VALUE pairs')
            [CompletionResult]::new('--envs', 'envs', [CompletionResultType]::ParameterName, 'Environment variables to store, as space separated KEY=VALUE pairs')
            [CompletionResult]::new('-g', 'g', [CompletionResultType]::ParameterName, 'Encrypt with this GPG key instead of a passphrase')
            [CompletionResult]::new('--gpg-key-fingerprint', 'gpg-key-fingerprint', [CompletionResultType]::ParameterName, 'Encrypt with this GPG key instead of a passphrase')
            [CompletionResult]::new('-c', 'c', [CompletionResultType]::ParameterName, 'Prompt for a comment for each environment variable')
            [CompletionResult]::new('--add-comments', 'add-comments', [CompletionResultType]::ParameterName, 'Prompt for a comment for each environment variable')
            [CompletionResult]::new('-x', 'x', [CompletionResultType]::ParameterName, 'Prompt for an expiration date for each environment variable')
            [CompletionResult]::new('--add-expiration-date', 'add-expiration-date', [CompletionResultType]::ParameterName, 'Prompt for an expiration date for each environment variable')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'envio;add' {
            [CompletionResult]::new('-e', 'e', [CompletionResultType]::ParameterName, 'Environment variables to add, as space separated KEY=VALUE pairs')
            [CompletionResult]::new('--envs', 'envs', [CompletionResultType]::ParameterName, 'Environment variables to add, as space separated KEY=VALUE pairs')
            [CompletionResult]::new('-c', 'c', [CompletionResultType]::ParameterName, 'Prompt for a comment for each environment variable')
            [CompletionResult]::new('--add-comments', 'add-comments', [CompletionResultType]::ParameterName, 'Prompt for a comment for each environment variable')
            [CompletionResult]::new('-x', 'x', [CompletionResultType]::ParameterName, 'Prompt for an expiration date for each environment variable')
            [CompletionResult]::new('--add-expiration-date', 'add-expiration-date', [CompletionResultType]::ParameterName, 'Prompt for an expiration date for each environment variable')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            envio list --profiles --no-pretty-print 2>$null | ForEach-Object {
                [CompletionResult]::new($_, $_, [CompletionResultType]::ParameterValue, $_)
            }
            break
        }
        'envio;load' {
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            envio list --profiles --no-pretty-print 2>$null | ForEach-Object {
                [CompletionResult]::new($_, $_, [CompletionResultType]::ParameterValue, $_)
            }
            break
        }
        'envio;unload' {
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            envio list --profiles --no-pretty-print 2>$null | ForEach-Object {
                [CompletionResult]::new($_, $_, [CompletionResultType]::ParameterValue, $_)
            }
            break
        }
        'envio;launch' {
            [CompletionResult]::new('-c', 'c', [CompletionResultType]::ParameterName, 'Command to run, as a single string')
            [CompletionResult]::new('--command', 'command', [CompletionResultType]::ParameterName, 'Command to run, as a single string')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            envio list --profiles --no-pretty-print 2>$null | ForEach-Object {
                [CompletionResult]::new($_, $_, [CompletionResultType]::ParameterValue, $_)
            }
            break
        }
        'envio;remove' {
            [CompletionResult]::new('-e', 'e', [CompletionResultType]::ParameterName, 'Names of the environment variables to remove; omit to delete the whole profile')
            [CompletionResult]::new('--envs-to-remove', 'envs-to-remove', [CompletionResultType]::ParameterName, 'Names of the environment variables to remove; omit to delete the whole profile')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            envio list --profiles --no-pretty-print 2>$null | ForEach-Object {
                [CompletionResult]::new($_, $_, [CompletionResultType]::ParameterValue, $_)
            }
            break
        }
        'envio;list' {
            [CompletionResult]::new('-n', 'n', [CompletionResultType]::ParameterName, 'Name of the profile whose environment variables to list')
            [CompletionResult]::new('--profile-name', 'profile-name', [CompletionResultType]::ParameterName, 'Name of the profile whose environment variables to list')
            [CompletionResult]::new('-p', 'p', [CompletionResultType]::ParameterName, 'List the stored profiles instead of environment variables')
            [CompletionResult]::new('--profiles', 'profiles', [CompletionResultType]::ParameterName, 'List the stored profiles instead of environment variables')
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'Print plain lines instead of a formatted table')
            [CompletionResult]::new('--no-pretty-print', 'no-pretty-print', [CompletionResultType]::ParameterName, 'Print plain lines instead of a formatted table')
            [CompletionResult]::new('-c', 'c', [CompletionResultType]::ParameterName, 'Show the comment attached to each environment variable')
            [CompletionResult]::new('--display-comments', 'display-comments', [CompletionResultType]::ParameterName, 'Show the comment attached to each environment variable')
            [CompletionResult]::new('-x', 'x', [CompletionResultType]::ParameterName, 'Show the expiration date of each environment variable')
            [CompletionResult]::new('--display-expiration-date', 'display-expiration-date', [CompletionResultType]::ParameterName, 'Show the expiration date of each environment variable')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            $prev = if ($commandElements.Count -ge 2) { $commandElements[$commandElements.Count - 2].Value } else { '' }
            if ($prev -eq '-n' -or $prev -eq '--profile-name') {
                envio list --profiles --no-pretty-print 2>$null | ForEach-Object {
                    [CompletionResult]::new($_, $_, [CompletionResultType]::ParameterValue, $_)
                }
            }
            break
        }
        'envio;update' {
            [CompletionResult]::new('-e', 'e', [CompletionResultType]::ParameterName, 'Names of the environment variables to update, space separated')
            [CompletionResult]::new('--envs', 'envs', [CompletionResultType]::ParameterName, 'Names of the environment variables to update, space separated')
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'Prompt for a new value for each environment variable')
            [CompletionResult]::new('--update-values', 'update-values', [CompletionResultType]::ParameterName, 'Prompt for a new value for each environment variable')
            [CompletionResult]::new('-c', 'c', [CompletionResultType]::ParameterName, 'Prompt for a new comment for each environment variable')
            [CompletionResult]::new('--update-comments', 'update-comments', [CompletionResultType]::ParameterName, 'Prompt for a new comment for each environment variable')
            [CompletionResult]::new('-x', 'x', [CompletionResultType]::ParameterName, 'Prompt for a new expiration date for each environment variable')
            [CompletionResult]::new('--update-expiration-date', 'update-expiration-date', [CompletionResultType]::ParameterName, 'Prompt for a new expiration date for each environment variable')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            envio list --profiles --no-pretty-print 2>$null | ForEach-Object {
                [CompletionResult]::new($_, $_, [CompletionResultType]::ParameterValue, $_)
            }
            break
        }
        'envio;export' {
            [CompletionResult]::new('-f', 'f', [CompletionResultType]::ParameterName, 'Path to write to (defaults to .env)')
            [CompletionResult]::new('--file-to-export-to', 'file-to-export-to', [CompletionResultType]::ParameterName, 'Path to write to (defaults to .env)')
            [CompletionResult]::new('-e', 'e', [CompletionResultType]::ParameterName, 'Names of the environment variables to export; omit to export all')
            [CompletionResult]::new('--envs', 'envs', [CompletionResultType]::ParameterName, 'Names of the environment variables to export; omit to export all')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            envio list --profiles --no-pretty-print 2>$null | ForEach-Object {
                [CompletionResult]::new($_, $_, [CompletionResultType]::ParameterValue, $_)
            }
            break
        }
        'envio;import' {
            [CompletionResult]::new('-f', 'f', [CompletionResultType]::ParameterName, 'Path to a locally stored profile file')
            [CompletionResult]::new('--file-to-import-from', 'file-to-import-from', [CompletionResultType]::ParameterName, 'Path to a locally stored profile file')
            [CompletionResult]::new('-u', 'u', [CompletionResultType]::ParameterName, 'URL to download the profile from')
            [CompletionResult]::new('--url', 'url', [CompletionResultType]::ParameterName, 'URL to download the profile from')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'envio;sync' {
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('remote', 'remote', [CompletionResultType]::ParameterValue, 'Manage sync remotes')
            [CompletionResult]::new('push', 'push', [CompletionResultType]::ParameterValue, 'Upload profiles to the remote (all local profiles if none given)')
            [CompletionResult]::new('pull', 'pull', [CompletionResultType]::ParameterValue, 'Download profiles from the remote (all remote profiles if none given)')
            [CompletionResult]::new('status', 'status', [CompletionResultType]::ParameterValue, 'Show how each profile compares with the remote')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'envio;sync;remote' {
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('add', 'add', [CompletionResultType]::ParameterValue, 'Add a remote interactively')
            [CompletionResult]::new('list', 'list', [CompletionResultType]::ParameterValue, 'List configured remotes')
            [CompletionResult]::new('remove', 'remove', [CompletionResultType]::ParameterValue, 'Remove a remote')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'envio;sync;remote;add' {
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'envio;sync;remote;list' {
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'envio;sync;remote;remove' {
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'envio;sync;remote;help' {
            [CompletionResult]::new('add', 'add', [CompletionResultType]::ParameterValue, 'Add a remote interactively')
            [CompletionResult]::new('list', 'list', [CompletionResultType]::ParameterValue, 'List configured remotes')
            [CompletionResult]::new('remove', 'remove', [CompletionResultType]::ParameterValue, 'Remove a remote')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'envio;sync;remote;help;add' {
            break
        }
        'envio;sync;remote;help;list' {
            break
        }
        'envio;sync;remote;help;remove' {
            break
        }
        'envio;sync;remote;help;help' {
            break
        }
        'envio;sync;push' {
            [CompletionResult]::new('-r', 'r', [CompletionResultType]::ParameterName, 'Remote name from sync.toml')
            [CompletionResult]::new('--remote', 'remote', [CompletionResultType]::ParameterName, 'Remote name from sync.toml')
            [CompletionResult]::new('-f', 'f', [CompletionResultType]::ParameterName, 'Overwrite even if the remote changed')
            [CompletionResult]::new('--force', 'force', [CompletionResultType]::ParameterName, 'Overwrite even if the remote changed')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'envio;sync;pull' {
            [CompletionResult]::new('-r', 'r', [CompletionResultType]::ParameterName, 'Remote name from sync.toml')
            [CompletionResult]::new('--remote', 'remote', [CompletionResultType]::ParameterName, 'Remote name from sync.toml')
            [CompletionResult]::new('-f', 'f', [CompletionResultType]::ParameterName, 'Overwrite even if the local copy changed')
            [CompletionResult]::new('--force', 'force', [CompletionResultType]::ParameterName, 'Overwrite even if the local copy changed')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'envio;sync;status' {
            [CompletionResult]::new('-r', 'r', [CompletionResultType]::ParameterName, 'Remote name from sync.toml')
            [CompletionResult]::new('--remote', 'remote', [CompletionResultType]::ParameterName, 'Remote name from sync.toml')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'envio;sync;help' {
            [CompletionResult]::new('remote', 'remote', [CompletionResultType]::ParameterValue, 'Manage sync remotes')
            [CompletionResult]::new('push', 'push', [CompletionResultType]::ParameterValue, 'Upload profiles to the remote (all local profiles if none given)')
            [CompletionResult]::new('pull', 'pull', [CompletionResultType]::ParameterValue, 'Download profiles from the remote (all remote profiles if none given)')
            [CompletionResult]::new('status', 'status', [CompletionResultType]::ParameterValue, 'Show how each profile compares with the remote')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'envio;sync;help;remote' {
            [CompletionResult]::new('add', 'add', [CompletionResultType]::ParameterValue, 'Add a remote interactively')
            [CompletionResult]::new('list', 'list', [CompletionResultType]::ParameterValue, 'List configured remotes')
            [CompletionResult]::new('remove', 'remove', [CompletionResultType]::ParameterValue, 'Remove a remote')
            break
        }
        'envio;sync;help;remote;add' {
            break
        }
        'envio;sync;help;remote;list' {
            break
        }
        'envio;sync;help;remote;remove' {
            break
        }
        'envio;sync;help;push' {
            break
        }
        'envio;sync;help;pull' {
            break
        }
        'envio;sync;help;status' {
            break
        }
        'envio;sync;help;help' {
            break
        }
        'envio;version' {
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'Also print the build target, timestamp and commit')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'Also print the build target, timestamp and commit')
            [CompletionResult]::new('--check', 'check', [CompletionResultType]::ParameterName, 'Check for a newer release (requires network)')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'envio;completion' {
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'envio;help' {
            [CompletionResult]::new('create', 'create', [CompletionResultType]::ParameterValue, 'Create a new profile')
            [CompletionResult]::new('add', 'add', [CompletionResultType]::ParameterValue, 'Add envionment variables to a profile')
            [CompletionResult]::new('load', 'load', [CompletionResultType]::ParameterValue, 'Load all environment variables in a profile for use in your terminal sessions')
            [CompletionResult]::new('unload', 'unload', [CompletionResultType]::ParameterValue, 'Unload a profile')
            [CompletionResult]::new('launch', 'launch', [CompletionResultType]::ParameterValue, 'Run a command with the environment variables from a profile')
            [CompletionResult]::new('remove', 'remove', [CompletionResultType]::ParameterValue, 'Remove a environment variable from a profile')
            [CompletionResult]::new('list', 'list', [CompletionResultType]::ParameterValue, 'List all the environment variables in a profile or all the profiles currenty stored')
            [CompletionResult]::new('update', 'update', [CompletionResultType]::ParameterValue, 'Update environment variables in a profile')
            [CompletionResult]::new('export', 'export', [CompletionResultType]::ParameterValue, 'Export a profile to a file if no file is specified it will be exported to a file named .env')
            [CompletionResult]::new('import', 'import', [CompletionResultType]::ParameterValue, 'Download a profile over the internet and import it into the system or import a locally stored profile into your current envio installation')
            [CompletionResult]::new('sync', 'sync', [CompletionResultType]::ParameterValue, 'Push and pull encrypted profiles to a remote store (S3, Google Drive, or a directory)')
            [CompletionResult]::new('version', 'version', [CompletionResultType]::ParameterValue, 'Print the version')
            [CompletionResult]::new('completion', 'completion', [CompletionResultType]::ParameterValue, 'Generate shell completion scripts')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'envio;help;create' {
            break
        }
        'envio;help;add' {
            break
        }
        'envio;help;load' {
            break
        }
        'envio;help;unload' {
            break
        }
        'envio;help;launch' {
            break
        }
        'envio;help;remove' {
            break
        }
        'envio;help;list' {
            break
        }
        'envio;help;update' {
            break
        }
        'envio;help;export' {
            break
        }
        'envio;help;import' {
            break
        }
        'envio;help;sync' {
            [CompletionResult]::new('remote', 'remote', [CompletionResultType]::ParameterValue, 'Manage sync remotes')
            [CompletionResult]::new('push', 'push', [CompletionResultType]::ParameterValue, 'Upload profiles to the remote (all local profiles if none given)')
            [CompletionResult]::new('pull', 'pull', [CompletionResultType]::ParameterValue, 'Download profiles from the remote (all remote profiles if none given)')
            [CompletionResult]::new('status', 'status', [CompletionResultType]::ParameterValue, 'Show how each profile compares with the remote')
            break
        }
        'envio;help;sync;remote' {
            [CompletionResult]::new('add', 'add', [CompletionResultType]::ParameterValue, 'Add a remote interactively')
            [CompletionResult]::new('list', 'list', [CompletionResultType]::ParameterValue, 'List configured remotes')
            [CompletionResult]::new('remove', 'remove', [CompletionResultType]::ParameterValue, 'Remove a remote')
            break
        }
        'envio;help;sync;remote;add' {
            break
        }
        'envio;help;sync;remote;list' {
            break
        }
        'envio;help;sync;remote;remove' {
            break
        }
        'envio;help;sync;push' {
            break
        }
        'envio;help;sync;pull' {
            break
        }
        'envio;help;sync;status' {
            break
        }
        'envio;help;version' {
            break
        }
        'envio;help;completion' {
            break
        }
        'envio;help;help' {
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}

# envio: dynamic profile completion END
