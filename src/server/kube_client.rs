//! In-cluster kube operations: ConfigMap read/patch, Deployment annotation
//! bump (to trigger asb pod restart), Job create + pod log streaming.

use std::collections::BTreeMap;

use anyhow::{Context, Result, anyhow};
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::batch::v1::Job;
use k8s_openapi::api::core::v1::{ConfigMap, Pod};
use kube::Client;
use kube::api::{Api, ListParams, LogParams, Patch, PatchParams, PostParams};
use serde_json::json;

#[derive(Clone)]
pub struct KubeClient {
    client: Client,
}

impl std::fmt::Debug for KubeClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KubeClient").finish_non_exhaustive()
    }
}

impl KubeClient {
    pub async fn try_in_cluster() -> Result<Self> {
        // Picks up the in-cluster ServiceAccount, or falls back to ~/.kube/config.
        let client = Client::try_default().await.context("init kube client")?;
        Ok(Self { client })
    }

    /// Clone of the underlying kube `Client`, for callers that want to build
    /// their own typed `Api<T>` rather than going through the wrapper methods.
    pub fn client(&self) -> Client {
        self.client.clone()
    }

    pub async fn read_configmap(&self, namespace: &str, name: &str) -> Result<ConfigMap> {
        let api: Api<ConfigMap> = Api::namespaced(self.client.clone(), namespace);
        api.get(name)
            .await
            .map_err(|e| anyhow!("get cm {namespace}/{name}: {e}"))
    }

    pub async fn write_configmap_data(
        &self,
        namespace: &str,
        name: &str,
        key: &str,
        value: &str,
    ) -> Result<()> {
        let api: Api<ConfigMap> = Api::namespaced(self.client.clone(), namespace);
        let patch = json!({
            "data": { key: value }
        });
        api.patch(
            name,
            &PatchParams::apply("eigenwallet-admin").force(),
            &Patch::Apply(patch),
        )
        .await
        .map(|_| ())
        .map_err(|e| anyhow!("patch cm {namespace}/{name}: {e}"))
    }

    /// Bump `spec.template.metadata.annotations["config-version"]` on the
    /// Deployment, which triggers a rolling restart of the pod that mounts
    /// the ConfigMap.
    pub async fn bump_deployment_annotation(
        &self,
        namespace: &str,
        name: &str,
        key: &str,
        value: &str,
    ) -> Result<()> {
        let api: Api<Deployment> = Api::namespaced(self.client.clone(), namespace);
        let patch = json!({
            "spec": {
                "template": {
                    "metadata": {
                        "annotations": { key: value }
                    }
                }
            }
        });
        api.patch(name, &PatchParams::default(), &Patch::Merge(&patch))
            .await
            .map(|_| ())
            .map_err(|e| anyhow!("patch deployment {namespace}/{name}: {e}"))
    }

    pub async fn deployment_ready(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<DeploymentReadiness> {
        let api: Api<Deployment> = Api::namespaced(self.client.clone(), namespace);
        let d = api.get(name).await.map_err(|e| anyhow!("get dep: {e}"))?;
        let status = d.status.unwrap_or_default();
        Ok(DeploymentReadiness {
            ready_replicas: status.ready_replicas.unwrap_or(0),
            replicas: status.replicas.unwrap_or(0),
            observed_generation: status.observed_generation.unwrap_or(0),
            updated_replicas: status.updated_replicas.unwrap_or(0),
        })
    }

    pub async fn list_pods(&self, namespace: &str) -> Result<Vec<Pod>> {
        let api: Api<Pod> = Api::namespaced(self.client.clone(), namespace);
        let lp = ListParams::default();
        let pods = api.list(&lp).await.map_err(|e| anyhow!("list pods: {e}"))?;
        Ok(pods.items)
    }

    pub async fn create_job(&self, namespace: &str, job: &Job) -> Result<Job> {
        let api: Api<Job> = Api::namespaced(self.client.clone(), namespace);
        api.create(&PostParams::default(), job)
            .await
            .map_err(|e| anyhow!("create job: {e}"))
    }

    pub async fn get_job(&self, namespace: &str, name: &str) -> Result<Job> {
        let api: Api<Job> = Api::namespaced(self.client.clone(), namespace);
        api.get(name).await.map_err(|e| anyhow!("get job: {e}"))
    }

    pub async fn job_pod_logs(&self, namespace: &str, job_name: &str) -> Result<String> {
        let pods: Api<Pod> = Api::namespaced(self.client.clone(), namespace);
        let lp = ListParams::default().labels(&format!("job-name={job_name}"));
        let plist = pods
            .list(&lp)
            .await
            .map_err(|e| anyhow!("list job pods: {e}"))?;
        let Some(pod) = plist.items.into_iter().next() else {
            return Err(anyhow!("no pod for job {job_name}"));
        };
        let pod_name = pod
            .metadata
            .name
            .ok_or_else(|| anyhow!("pod has no name"))?;
        let logs = pods
            .logs(&pod_name, &LogParams::default())
            .await
            .map_err(|e| anyhow!("logs {pod_name}: {e}"))?;
        Ok(logs)
    }

    /// Build a list-sellers Job. Args are kept aligned with the working
    /// `swap-cli-scan` pod in the eigenwallet namespace — rendezvous points are
    /// read from the binary's defaults / its mounted config, *not* passed as
    /// CLI args. The scan needs Tor for onion seller reachability.
    /// `_rendezvous_points` and `_my_peer_id` are retained on the API surface
    /// for future use but not currently consumed.
    pub fn build_scan_job(
        &self,
        namespace: &str,
        image: &str,
        _rendezvous_points: &[String],
        _my_peer_id: Option<&str>,
    ) -> Job {
        let args = vec![
            "-d".to_string(),
            "/tmp/swap-cli".to_string(),
            "list-sellers".to_string(),
            "--electrum-rpc".to_string(),
            "tcp://electrs:50001".to_string(),
            "--enable-tor".to_string(),
            "--wait-seconds".to_string(),
            "60".to_string(),
        ];

        let annotations: BTreeMap<String, String> = BTreeMap::new();
        let job: Job = serde_json::from_value(json!({
            "apiVersion": "batch/v1",
            "kind": "Job",
            "metadata": {
                "generateName": "ewa-scan-",
                "namespace": namespace,
                "annotations": annotations,
                "labels": { "app.kubernetes.io/managed-by": "eigenwallet-admin", "ewa.kind": "scan" }
            },
            "spec": {
                "ttlSecondsAfterFinished": 600,
                "backoffLimit": 0,
                "activeDeadlineSeconds": 600,
                "template": {
                    "spec": {
                        "restartPolicy": "Never",
                        "containers": [{
                            "name": "scan",
                            "image": image,
                            "args": args,
                        }]
                    }
                }
            }
        }))
        .expect("static scan job json");
        job
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DeploymentReadiness {
    pub ready_replicas: i32,
    pub replicas: i32,
    pub observed_generation: i64,
    pub updated_replicas: i32,
}

impl DeploymentReadiness {
    pub fn is_ready(&self) -> bool {
        self.replicas > 0 && self.ready_replicas >= self.replicas
    }
}
