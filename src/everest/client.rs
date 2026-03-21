#![allow(dead_code, unused_variables)]
use reqwest::header::{ACCEPT, ACCEPT_ENCODING, HeaderName, HeaderValue};
use serde::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EverestBuild {
    pub date: String,
    /// Four digits number of version. This is not follows semantic versiong.
    pub version: u32,
    pub author: String,
    pub description: String,
    pub branch: Branch,
    pub commit: String,
    pub is_native: Option<bool>,

    /// Download link for `main.zip`
    pub main_download: String,
    pub main_file_size: u64,
}

/// Build branch variant.
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Branch {
    Stable,
    Dev,
    Beta,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] reqwest::Error),
}

pub struct EverestClient {
    http_client: reqwest::Client,
}

impl EverestClient {
    // FIXME: This is mirror URL. We should implement determin logic for actual URL based on user's preference.
    const ENDPOINT_EVEREST_UPDATE: &str =
        "https://everestapi.github.io/updatermirror/everest_versions.json";

    pub fn new() -> Self {
        Self {
            // TODO: build proper client
            http_client: reqwest::Client::new(),
        }
    }

    /// Returns API endpoint.
    pub async fn get_url(&self, is_mirror: bool) -> Result<String, reqwest::Error> {
        // TODO: Returns Url instead of String
        if is_mirror {
            info!("Using mirror for the Everest updater database");
            return Ok("https://everestapi.github.io/updatermirror/everest_versions.json".into());
        }

        // TODO: Extract this logic into a function
        info!("Fetching Everest updater database URL");
        Ok(self
            .http_client
            .get("https://everestapi.github.io/everestupdater.txt")
            .header(ACCEPT, HeaderValue::from_static("application/json"))
            .header(ACCEPT_ENCODING, HeaderValue::from_static("gzip"))
            .send()
            .await?
            .text()
            .await?
            .trim()
            .into())
        // TODO: Adds Url crate to append mandantory query
        // url.query_pairs_mut()
        //     .append_pair("supportsNativeBuilds", "true");
    }

    // 1. JSONを叩いて更新リスト（構造体）を返す
    pub async fn fetch_update_list(&self, url: &str) -> Result<Vec<EverestBuild>, Error> {
        let response = self.http_client.get(url).send().await?;
        let builds: Vec<EverestBuild> = response.json().await?;
        Ok(builds)
    }

    // 2. 指定したURL（またはバージョン）からファイルをダウンロードする
    pub async fn download_everest(&self, version: &str) -> Result<Vec<u8>, Error> {
        // NOTE: ダウンロードしたディレクトリの中にバージョン情報を格納したファイルを配置する: `update-build.txt`
        // NOTE: GUI の `EverestUpdater.cs` と CLI の `MiniInstaller/` の両方の処理を考慮しなければならない
        todo!()
    }

    // TODO: `JSON` の中身を表示するだけの簡易的なデバッグ関数を実装 `List SubCommand`
    // TODO: `branch` で filter して、最新の `version` を特定
}
