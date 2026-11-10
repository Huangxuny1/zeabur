//! The container runtimes a team install can turn on beside the distribution's default one.

use serde::Serialize;
use serde_json::{Value, json};

use crate::PlanError;
use crate::cluster::{Distribution, HostFile};
pub use crate::pins::GVISOR;
use crate::plan::{Download, MANAGER};

/// A runtime that runs Pods beside the distribution's default one.
pub trait Runtime: Sync {
    /// The key people configure, `gvisor`.
    fn id(&self) -> &'static str;
    /// The name people read, `gVisor`.
    fn display_name(&self) -> &'static str;
    /// The `RuntimeClass` handler, `runsc`.
    fn handler(&self) -> &'static str;
    /// The release tag.
    fn version(&self) -> &'static str;
    /// The binary whose `--version` names the installed release.
    fn binary(&self) -> &'static str;
    /// The paths the release installs.
    fn paths(&self) -> &'static [&'static str];
    /// The directory the release unpacks into.
    fn install_dir(&self) -> &'static str;
    /// Checks that a configuration can use the runtime.
    ///
    /// # Errors
    ///
    /// When the install is personal or names a release other than the pinned one.
    fn check(&self, install: Install<'_>) -> Result<(), PlanError>;
    /// The download and the `RuntimeClass` for `arch`.
    ///
    /// # Errors
    ///
    /// When the runtime has no release for the architecture.
    fn plan(&self, arch: &str) -> Result<RuntimePlan, PlanError>;
    /// The containerd drop-in that registers the runtime.
    fn containerd_file(&self, distribution: Distribution) -> HostFile;
}

/// The install a configuration describes.
#[derive(Debug, Clone, Copy)]
pub enum Install<'a> {
    /// A personal install on `KubeSolo`.
    Personal,
    /// A team install, with the release the configuration chooses instead of the pinned one.
    Team {
        /// The release the configuration chooses, if any.
        version: Option<&'a str>,
    },
}

/// A runtime the plan installs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePlan {
    /// The key people configure.
    pub id: String,
    /// The name people read.
    pub display_name: String,
    /// The release tag.
    pub version: String,
    /// The release's archive.
    pub download: Download,
    /// The binary whose `--version` names the release.
    pub binary: String,
    /// The paths the release installs.
    pub paths: Vec<String>,
    /// The directory the release unpacks into.
    pub install_dir: String,
    /// The `RuntimeClass` workloads name.
    pub runtime_class: Value,
}

/// The runtime a configuration turns on.
#[must_use]
pub fn enabled(on: bool) -> Option<&'static dyn Runtime> {
    on.then_some(&GVISOR)
}

/// The runtime plan for `arch`, when the configuration turns a runtime on.
///
/// # Errors
///
/// When the runtime has no release for the architecture.
pub fn plan(on: bool, arch: &str) -> Result<Option<RuntimePlan>, PlanError> {
    enabled(on).map(|runtime| runtime.plan(arch)).transpose()
}

/// The runtime with `id`, when this installer knows it.
#[must_use]
pub fn by_id(id: &str) -> Option<&'static dyn Runtime> {
    (GVISOR.id() == id).then_some(&GVISOR)
}

impl Runtime for crate::pins::Gvisor {
    fn id(&self) -> &'static str {
        "gvisor"
    }

    fn display_name(&self) -> &'static str {
        "gVisor"
    }

    fn handler(&self) -> &'static str {
        "runsc"
    }

    fn version(&self) -> &'static str {
        self.version
    }

    fn binary(&self) -> &'static str {
        "/usr/local/bin/runsc"
    }

    fn paths(&self) -> &'static [&'static str] {
        &[
            "/usr/local/bin/runsc",
            "/usr/local/bin/containerd-shim-runsc-v1",
            "/usr/local/bin/gvisor-bin",
        ]
    }

    fn install_dir(&self) -> &'static str {
        "/usr/local/bin"
    }

    fn check(&self, install: Install<'_>) -> Result<(), PlanError> {
        match install {
            Install::Personal => Err(PlanError::Config(format!(
                "host.{} applies to a team install on K3s or RKE2; a personal install runs KubeSolo",
                self.id()
            ))),
            Install::Team { version: None } => Ok(()),
            Install::Team { version: Some(_) } => Err(PlanError::Config(format!(
                "host.{}: only the pinned K3s or RKE2 release is supported; remove k3sVersion or \
                 rke2Version",
                self.id()
            ))),
        }
    }

    fn plan(&self, arch: &str) -> Result<RuntimePlan, PlanError> {
        let download = self.tarball(arch).ok_or_else(|| {
            PlanError::Config(format!(
                "host.{}: {} has no release for {arch}; it supports amd64 and arm64",
                self.id(),
                self.display_name()
            ))
        })?;
        Ok(RuntimePlan {
            id: self.id().to_owned(),
            display_name: self.display_name().to_owned(),
            version: self.version().to_owned(),
            download: download.into(),
            binary: self.binary().to_owned(),
            paths: self.paths().iter().map(|path| (*path).to_owned()).collect(),
            install_dir: self.install_dir().to_owned(),
            runtime_class: json!({
                "apiVersion": "node.k8s.io/v1",
                "kind": "RuntimeClass",
                "metadata": {
                    "name": self.id(),
                    "labels": {"app.kubernetes.io/managed-by": MANAGER},
                },
                "handler": self.handler(),
            }),
        })
    }

    fn containerd_file(&self, distribution: Distribution) -> HostFile {
        let handler = self.handler();
        HostFile {
            path: format!(
                "{}/config-v3.toml.d/99-{}.toml",
                distribution.layout().containerd_dir,
                self.id()
            ),
            mode: 0o600,
            content: format!(
                "# Written by zeabur-installer; `zeabur-installer doctor` compares it.\n\
                 [plugins.'io.containerd.cri.v1.runtime'.containerd.runtimes.{handler}]\n\
                 runtime_type = \"io.containerd.{handler}.v1\"\n"
            ),
        }
    }
}
