#!/usr/bin/env bash
# Validate deterministic M8 LSP transcript fixture structure without adding a
# Cargo dependency. JSON::PP is part of the Perl toolchain already used by the
# benchmark script.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

perl <<'PERL'
use strict;
use warnings;
use JSON::PP;

my $root = "tests/lsp/m8-base";
opendir(my $dir, $root) or die "lsp fixtures: cannot open $root: $!\n";
my @files = sort grep { /\.json\z/ } readdir($dir);
closedir($dir);

die "lsp fixtures: no JSON fixtures in $root\n" unless @files;

my %seen;
my $count = 0;
for my $file (@files) {
    my $path = "$root/$file";
    open(my $fh, "<", $path) or die "lsp fixtures: cannot read $path: $!\n";
    local $/;
    my $text = <$fh>;
    close($fh);

    my $json = eval { decode_json($text) };
    die "lsp fixtures: $path is not valid JSON: $@\n" if $@;

    my ($name) = $file =~ /\A(.+)\.json\z/;
    fail($path, "schema must be keel-lsp-transcript/v1")
        unless scalar_value($json->{schema}) eq "keel-lsp-transcript/v1";
    fail($path, "case must match filename")
        unless scalar_value($json->{case}) eq $name;
    fail($path, "case must be numbered kebab-case")
        unless $name =~ /\A[0-9]{3}-[a-z0-9]+(?:-[a-z0-9]+)*\z/;
    fail($path, "duplicate case $name") if $seen{$name}++;
    fail($path, "description must be non-empty")
        unless scalar_value($json->{description}) ne "";
    fail($path, "messages must be a non-empty array")
        unless ref($json->{messages}) eq "ARRAY" && @{$json->{messages}};

    my $index = 0;
    for my $message (@{$json->{messages}}) {
        $index++;
        validate_message($path, $index, $message);
    }
    $count++;
}

print "lsp fixtures: ok ($count transcript(s))\n";

sub validate_message {
    my ($path, $index, $entry) = @_;
    fail($path, "message $index must be an object") unless ref($entry) eq "HASH";

    my $direction = scalar_value($entry->{direction});
    my $kind = scalar_value($entry->{kind});
    fail($path, "message $index has invalid direction")
        unless $direction eq "client" || $direction eq "server";
    fail($path, "message $index has invalid kind")
        unless $kind eq "request"
            || $kind eq "response"
            || $kind eq "notification"
            || $kind eq "raw-error";

    if ($direction eq "client") {
        fail($path, "message $index client entry must carry message or raw")
            unless exists($entry->{message}) || exists($entry->{raw});
    } else {
        fail($path, "message $index server entry must carry expect")
            unless exists($entry->{expect});
    }

    if (exists($entry->{message})) {
        validate_json_rpc($path, $index, $entry->{message});
        validate_request_or_notification($path, $index, $kind, $entry->{message});
    }
    if (exists($entry->{expect})) {
        validate_json_rpc($path, $index, $entry->{expect});
        validate_expectation($path, $index, $kind, $entry->{expect});
    }
    if ($kind eq "raw-error") {
        fail($path, "message $index raw-error must carry raw text")
            unless scalar_value($entry->{raw}) ne "";
        fail($path, "message $index raw-error must expect a JSON-RPC error")
            unless ref($entry->{expect}) eq "HASH" && ref($entry->{expect}->{error}) eq "HASH";
    }
}

sub validate_json_rpc {
    my ($path, $index, $object) = @_;
    fail($path, "message $index JSON-RPC payload must be an object")
        unless ref($object) eq "HASH";
    fail($path, "message $index jsonrpc must be 2.0")
        unless scalar_value($object->{jsonrpc}) eq "2.0";
}

sub validate_request_or_notification {
    my ($path, $index, $kind, $object) = @_;
    return if $kind eq "raw-error";
    fail($path, "message $index must carry method")
        unless scalar_value($object->{method}) ne "";
    if ($kind eq "request") {
        fail($path, "message $index request must carry id")
            unless exists($object->{id});
    }
    if ($kind eq "notification") {
        fail($path, "message $index notification must not carry id")
            if exists($object->{id});
    }
}

sub validate_expectation {
    my ($path, $index, $kind, $object) = @_;
    if ($kind eq "notification") {
        fail($path, "message $index expected notification must carry method")
            unless scalar_value($object->{method}) ne "";
    }
    if ($object->{method} && scalar_value($object->{method}) eq "textDocument/publishDiagnostics") {
        validate_diagnostics($path, $index, $object);
    }
    if (ref($object->{result}) eq "HASH" && ref($object->{result}->{capabilities}) eq "HASH") {
        validate_capabilities($path, $index, $object->{result}->{capabilities});
    }
}

sub validate_diagnostics {
    my ($path, $index, $object) = @_;
    my $params = $object->{params};
    fail($path, "message $index diagnostics params must be an object")
        unless ref($params) eq "HASH";
    fail($path, "message $index diagnostics uri must be non-empty")
        unless scalar_value($params->{uri}) ne "";
    fail($path, "message $index diagnostics must be an array")
        unless ref($params->{diagnostics}) eq "ARRAY";

    for my $diagnostic (@{$params->{diagnostics}}) {
        fail($path, "message $index diagnostic must have K#### code")
            unless scalar_value($diagnostic->{code}) =~ /\AK[0-9]{4}\z/;
        fail($path, "message $index diagnostic source must be keelc")
            unless scalar_value($diagnostic->{source}) eq "keelc";
        fail($path, "message $index diagnostic severity must be 1 or 2")
            unless number_value($diagnostic->{severity}) == 1
                || number_value($diagnostic->{severity}) == 2;
        validate_position($path, $index, $diagnostic->{range}->{start}, "start");
        validate_position($path, $index, $diagnostic->{range}->{end}, "end");
    }
}

sub validate_position {
    my ($path, $index, $position, $label) = @_;
    fail($path, "message $index range $label must be an object")
        unless ref($position) eq "HASH";
    fail($path, "message $index range $label line must be non-negative integer")
        unless integer_value($position->{line}) >= 0;
    fail($path, "message $index range $label character must be non-negative integer")
        unless integer_value($position->{character}) >= 0;
}

sub validate_capabilities {
    my ($path, $index, $capabilities) = @_;
    my @deferred = qw(
        referencesProvider
        documentFormattingProvider
        codeActionProvider
        workspaceSymbolProvider
        renameProvider
        inlayHintProvider
        semanticTokensProvider
    );
    for my $capability (@deferred) {
        fail($path, "message $index advertises deferred capability $capability")
            if exists($capabilities->{$capability});
    }
}

sub scalar_value {
    my ($value) = @_;
    return "" if !defined($value) || ref($value);
    return "$value";
}

sub number_value {
    my ($value) = @_;
    return -1 if !defined($value) || ref($value) || "$value" !~ /\A[0-9]+\z/;
    return int($value);
}

sub integer_value {
    my ($value) = @_;
    return -1 if !defined($value) || ref($value) || "$value" !~ /\A[0-9]+\z/;
    return int($value);
}

sub fail {
    my ($path, $message) = @_;
    die "lsp fixtures: $path: $message\n";
}
PERL
