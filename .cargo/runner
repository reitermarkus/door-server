#!/usr/bin/env bash

set -euo pipefail

user="$(yq e '.all.hosts[].ansible_user' inventory.yml)"
host="$(yq e '.all.hosts[].ansible_host' inventory.yml)"
ssh_host="${user}@${host}"
binary_name="$(basename "${1}")"
remote_path="/tmp/${binary_name}"

scp "${1}" "${ssh_host}:${remote_path}"
ssh "${ssh_host}" "sudo systemctl stop '${binary_name}'"
ssh -t "${ssh_host}" "sudo RUST_LOG='${RUST_LOG-}' '${remote_path}'"
