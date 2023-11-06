use std::{collections::HashSet, fs::File, path::PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::{commit_stats::CommitStatsRepositoryRefTargetOnCommitHistoryNodes, DateTime};

type CommitStats = CommitStatsRepositoryRefTargetOnCommitHistoryNodes;

pub trait CommitPack {
    type Commit;

    fn add_commit(&mut self, commit: Self::Commit) -> anyhow::Result<()>;
    fn save_curr_pack(&mut self) -> anyhow::Result<()>;
    fn load_pack(&mut self, pack_date: String) -> anyhow::Result<()>;
}

#[derive(Default, Deserialize, Serialize)]
pub struct DataState {
    // Only ever true on the very first time it runs without any data whatsoever already present.
    has_no_data: bool,

    latest_commit_oid: String,
    latest_commit_time: DateTime,

    earliest_commit_oid: String,
    earliest_commit_time: DateTime,

    #[serde(skip)]
    unsaved_latest_commit_oid: String,
    #[serde(skip)]
    unsaved_latest_commit_time: DateTime,

    #[serde(skip)]
    unsaved_earliest_commit_oid: String,
    #[serde(skip)]
    unsaved_earliest_commit_time: DateTime,

    // We keep it around so we can save the state whenever we want without the caller having to keep a reference to the path.
    #[serde(skip)]
    state_file_path: PathBuf,

    #[serde(skip)]
    data_dir: PathBuf,
    #[serde(skip)]
    curr_raw_data_pack_date: String,
    #[serde(skip)]
    curr_raw_data_pack: Vec<CommitStats>,
    #[serde(skip)]
    curr_raw_data_pack_index: HashSet<String>,
}

impl DataState {
    fn reset_unsaved_state(&mut self) {
        self.unsaved_latest_commit_oid = self.latest_commit_oid.clone();
        self.unsaved_latest_commit_time = self.latest_commit_time;
        self.unsaved_earliest_commit_oid = self.earliest_commit_oid.clone();
        self.unsaved_earliest_commit_time = self.earliest_commit_time;
    }

    pub fn from_file(path: PathBuf) -> anyhow::Result<Self> {
        let state_file = File::open(&path).context("Failed to read from state file.")?;

        let mut state: Self =
            serde_json::from_reader(state_file).context("Failed to deserialize the state.")?;
        state.reset_unsaved_state();
        state.state_file_path = path;
        Ok(state)
    }

    pub fn set_data_dir(&mut self, data_dir: PathBuf) {
        self.data_dir = data_dir;
    }

    pub fn latest_commit_time(&self) -> Option<String> {
        if self.has_no_data {
            None
        } else {
            Some(self.latest_commit_time.to_rfc3339())
        }
    }

    pub fn earliest_commit_time(&self) -> Option<String> {
        if self.has_no_data {
            None
        } else {
            Some(self.earliest_commit_time.to_rfc3339())
        }
    }

    pub fn curr_pack_date(&self) -> String {
        self.curr_raw_data_pack_date.clone()
    }

    pub fn save(&mut self) -> anyhow::Result<()> {
        self.save_to_path(self.state_file_path.clone())
    }

    pub fn save_to_path(&mut self, path: PathBuf) -> anyhow::Result<()> {
        let state_file = File::options().write(true).open(&path).with_context(|| {
            format!(
                "Failed to open the state file '{}' for writing.",
                path.to_string_lossy()
            )
        })?;

        self.latest_commit_oid = self.unsaved_latest_commit_oid.clone();
        self.latest_commit_time = self.unsaved_latest_commit_time;
        self.earliest_commit_oid = self.unsaved_earliest_commit_oid.clone();
        self.earliest_commit_time = self.unsaved_earliest_commit_time;

        serde_json::to_writer(state_file, &self).with_context(|| {
            format!(
                "Failed to serialize the state to path '{}'.",
                path.to_string_lossy()
            )
        })?;
        Ok(())
    }

    fn pack_path(&self, pack_date: &String) -> PathBuf {
        self.data_dir.join(format!("raw_{}.json", pack_date))
    }

    fn curr_pack_path(&self) -> PathBuf {
        self.pack_path(&self.curr_raw_data_pack_date)
    }

    fn switch_to_pack(&mut self, pack_date: String) -> anyhow::Result<()> {
        if pack_date == self.curr_raw_data_pack_date {
            return Ok(());
        }

        self.save_curr_pack()?;
        self.load_pack(pack_date)
    }
}

impl CommitPack for DataState {
    type Commit = CommitStatsRepositoryRefTargetOnCommitHistoryNodes;

    fn add_commit(&mut self, commit: Self::Commit) -> anyhow::Result<()> {
        let commit_pack_date = commit.committed_date.format("%Y-%m").to_string();

        if commit_pack_date != self.curr_raw_data_pack_date {
            self.switch_to_pack(commit_pack_date)?;
        }

        if self.curr_raw_data_pack_index.insert(commit.oid.clone()) {
            if self.unsaved_earliest_commit_time > commit.committed_date {
                self.unsaved_earliest_commit_time = commit.committed_date;
                self.unsaved_earliest_commit_oid = commit.oid.clone();
            }
            if self.unsaved_latest_commit_time < commit.committed_date {
                self.unsaved_latest_commit_time = commit.committed_date;
                self.unsaved_latest_commit_oid = commit.oid.clone();
            }

            self.curr_raw_data_pack.push(commit);
        }

        Ok(())
    }

    fn save_curr_pack(&mut self) -> anyhow::Result<()> {
        if self.curr_raw_data_pack.is_empty() {
            return Ok(());
        }

        println!("Saving pack with date {}.", self.curr_raw_data_pack_date);

        let curr_pack_file = File::options()
            .create(true)
            .write(true)
            .open(self.curr_pack_path())
            .with_context(|| {
                format!(
                    "Failed to open or create the commit pack file '{}' for writing.",
                    self.curr_pack_path().to_string_lossy()
                )
            })?;

        serde_json::to_writer(&curr_pack_file, &self.curr_raw_data_pack)
            .context("Failed to serialize the current commit pack.")?;
        self.has_no_data = false;
        self.save()?;

        Ok(())
    }

    fn load_pack(&mut self, pack_date: String) -> anyhow::Result<()> {
        let new_pack_path = self.pack_path(&pack_date);
        let new_pack_file = File::options()
            .create(false)
            .read(true)
            .open(&new_pack_path);

        println!("Opening pack with date {}.", pack_date);

        match new_pack_file {
            Ok(f) => {
                let pack: Vec<CommitStats> = serde_json::from_reader(f).with_context(|| {
                    format!(
                        "Failed to deserialize the pack file '{}'.",
                        new_pack_path.to_string_lossy()
                    )
                })?;
                let mut index = HashSet::with_capacity(pack.len());

                pack.iter().for_each(|c| {
                    index.insert(c.oid.clone());
                });

                self.curr_raw_data_pack = pack;
                self.curr_raw_data_pack_date = pack_date;
                self.curr_raw_data_pack_index = index;
                Ok(())
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                self.curr_raw_data_pack = Vec::new();
                self.curr_raw_data_pack_date = pack_date;
                self.curr_raw_data_pack_index = HashSet::new();
                Ok(())
            }
            Err(err) => Err(err).with_context(|| {
                format!(
                    "Failed to open the commit pack file '{}' for reading.",
                    new_pack_path.to_string_lossy()
                )
            }),
        }
    }
}
