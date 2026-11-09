---
default: minor
---

# The installer can install gVisor

`host.gvisor: true` installs the pinned gVisor release on every node of a team install on K3s or
RKE2 and creates the `gvisor` RuntimeClass. Workloads opt in with `runtimeClassName: gvisor`.
`apply` installs the binaries before the first start, or restarts a running cluster once, and
`doctor` checks the binaries, the containerd drop-in and the RuntimeClass. `node add` gives a
joining node the same runtime. A personal install on KubeSolo doesn't accept the setting.
