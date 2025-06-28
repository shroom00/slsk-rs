use serde::{Deserialize, Serialize};
use smol::block_on;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::{
    messages::{Directory, File, FileAttribute, SharedFileListResponse},
    utils::file_is_hidden,
};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(from = "DiskIndexDeser")]
pub(crate) struct DiskIndex {
    #[serde(skip)]
    pool: SqlitePool,
    save_dir: PathBuf,
    root_folders: Vec<(PathBuf, String, bool)>,
    #[serde(skip)]
    folder_aliases: HashMap<PathBuf, String>, // real_path -> alias
    #[serde(skip)]
    alias_to_path: HashMap<String, PathBuf>, // alias -> real_path
}

#[derive(Deserialize)]
struct DiskIndexDeser {
    save_dir: PathBuf,
    root_folders: Vec<(PathBuf, String, bool)>,
}

impl From<DiskIndexDeser> for DiskIndex {
    fn from(deser: DiskIndexDeser) -> Self {
        let mut index = block_on(DiskIndex::new(deser.save_dir)).unwrap();
        if block_on(index.get_folder_count()).unwrap_or_default() == 0 {
            for (folder_path, alias, is_buddy_only) in deser.root_folders {
                let _ = block_on(index.index_folder(folder_path, &alias, is_buddy_only));
            }
        }
        index
    }
}

impl DiskIndex {
    pub(crate) fn setup() -> Self {
        block_on(Self::new(".shares")).unwrap()
    }

    /// Create a new DiskIndex with optimized database schema
    pub(crate) async fn new(
        save_dir: impl AsRef<std::path::Path>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let save_dir = save_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&save_dir)?;
        let db_path = save_dir.join("index.db");

        let pool = SqlitePool::connect_with(
            SqliteConnectOptions::new()
                .create_if_missing(true)
                .filename(db_path),
        )
        .await?;

        // Initialize the database schema
        Self::initialize_database(&pool).await?;

        // Load existing folder mappings
        let (folder_aliases, alias_to_path, root_folders) =
            Self::load_folder_mappings(&pool).await?;

        Ok(DiskIndex {
            pool,
            save_dir,
            folder_aliases,
            alias_to_path,
            root_folders,
        })
    }

    /// Initialize the optimized database schema
    async fn initialize_database(pool: &SqlitePool) -> Result<(), sqlx::Error> {
        // Enable WAL mode for better concurrent performance
        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(pool)
            .await?;

        // Enable foreign key constraints
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(pool)
            .await?;

        // Create tables in a single transaction
        let mut tx = pool.begin().await?;

        // Folders table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS folders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                alias TEXT UNIQUE NOT NULL,
                is_buddy_only BOOLEAN NOT NULL DEFAULT 0
            )
            "#,
        )
        .execute(&mut *tx)
        .await?;

        // Root Folders table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS root_folders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT UNIQUE NOT NULL,
                alias TEXT UNIQUE NOT NULL,
                is_buddy_only BOOLEAN NOT NULL DEFAULT 0
            )
            "#,
        )
        .execute(&mut *tx)
        .await?;

        // Files table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                folder_id INTEGER NOT NULL,
                filename TEXT NOT NULL,
                modified_time INTEGER NOT NULL,
                UNIQUE(folder_id, filename),
                FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&mut *tx)
        .await?;

        // File metadata table - starts empty, populated on search
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS file_metadata (
                file_id INTEGER PRIMARY KEY,
                bitrate INTEGER,        -- kbps
                duration REAL,          -- seconds
                vbr BOOLEAN,           -- is/is not VBR
                sample_rate INTEGER,    -- Hz
                bit_depth INTEGER,      -- bits
                filesize INTEGER,
                FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&mut *tx)
        .await?;

        // Terms table for fast lookups
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS terms (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                term TEXT UNIQUE NOT NULL
            )
            "#,
        )
        .execute(&mut *tx)
        .await?;

        // File-terms junction table for fast lookups
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS file_terms (
                file_id INTEGER NOT NULL,
                term_id INTEGER NOT NULL,
                PRIMARY KEY (file_id, term_id),
                FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE,
                FOREIGN KEY (term_id) REFERENCES terms(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_folders_alias ON folders(alias)")
            .execute(&mut *tx)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_files_folder_id ON files(folder_id)")
            .execute(&mut *tx)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_files_filename ON files(filename)")
            .execute(&mut *tx)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_files_modified ON files(modified_time)")
            .execute(&mut *tx)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_terms_term ON terms(term)")
            .execute(&mut *tx)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_file_terms_term_id ON file_terms(term_id)")
            .execute(&mut *tx)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_file_terms_file_id ON file_terms(file_id)")
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Load existing folder mappings from database
    async fn load_folder_mappings(
        pool: &SqlitePool,
    ) -> Result<
        (
            HashMap<PathBuf, String>,
            HashMap<String, PathBuf>,
            Vec<(PathBuf, String, bool)>,
        ),
        sqlx::Error,
    > {
        let rows = sqlx::query_as::<_, (String, String, bool)>(
            "SELECT path, alias, is_buddy_only FROM root_folders",
        )
        .fetch_all(pool)
        .await?;
        let row_len = rows.len();

        let mut root_folders = Vec::with_capacity(row_len);
        let mut folder_aliases = HashMap::new();
        let mut alias_to_path = HashMap::new();

        for (path_str, alias, is_buddy_only) in rows.into_iter() {
            let path = PathBuf::from(path_str);
            root_folders.push((path.clone(), alias.clone(), is_buddy_only));
            folder_aliases.insert(path.clone(), alias.clone());
            alias_to_path.insert(alias, path);
        }

        Ok((folder_aliases, alias_to_path, root_folders))
    }

    pub(crate) fn root_folders(&self) -> &Vec<(PathBuf, String, bool)> {
        &self.root_folders
    }

    pub(crate) async fn file_list(&self) -> Result<SharedFileListResponse, sqlx::Error> {
        let mut directories: Vec<Directory> =
            Vec::with_capacity(self.get_folder_count().await? as usize);
        let mut priv_directories: Vec<Directory> =
            Vec::with_capacity(self.get_folder_count().await? as usize);
        let mut dir_files = Vec::new();
        let mut previous_id = -1;
        for (file_id, folder_id, filename, alias, is_buddy_only, bitrate, duration, vbr, sample_rate, bit_depth, file_size) in
            sqlx::query_as::<_, (i64, i64, String, String, bool, Option<u32>, Option<f64>, Option<bool>, Option<u32>, Option<u32>, Option<u64>)>(
                r#"
            SELECT f.id, folder_id, filename, alias, is_buddy_only, bitrate, duration, vbr, sample_rate, bit_depth, filesize
            FROM folders fo
            JOIN files f ON f.folder_id = fo.id
            LEFT JOIN file_metadata fm ON f.id = fm.file_id
            ORDER BY f.folder_id
            "#,
            )
            .fetch_all(&self.pool)
            .await?
        {
            let (file_size, attributes) = match file_size {
                Some(file_size) => (file_size, 
                    FileAttribute::from_parts(bitrate, duration.map(|d| d.round() as u32), vbr, sample_rate, bit_depth)),
                None => {
                    match self
                    .get_file_metadata(file_id)
                    .await {
                        Ok(Some((attributes, file_size))) => 
                        (file_size.unwrap_or_default(), attributes),
                        _ => (0, Vec::new()),
                    }
            }};
            if previous_id != folder_id {
                previous_id = folder_id;
                if !dir_files.is_empty().clone() {
                    let dir = Directory {
                        path: alias.clone(),
                        files: dir_files,
                    };
                    if is_buddy_only {
                        priv_directories.push(dir);
                    } else {
                        directories.push(dir);
                    };
                    dir_files = Vec::new();
                }
            };
            let extension = filename
                .rsplit_once('.')
                .map(|(_, ext)| ext.to_string())
                .unwrap_or_default();
            dir_files.push(File {
                code: 1,
                filename,
                file_size,
                extension,
                attributes,
            });
        }
        let file_list = SharedFileListResponse {
            directories,
            _unknown_0: 0,
            priv_directories,
        };
        Ok(file_list)
    }

    /// Search for files by filename or terms
    /// Returns a list of files and private files, along with their actual paths
    pub(crate) async fn search(
        &self,
        query: &str,
    ) -> Result<(Vec<(File, PathBuf)>, Vec<(File, PathBuf)>), Box<dyn std::error::Error>> {
        let search_pattern = format!("%{}%", query.to_lowercase());

        // Search both by filename and by terms
        let rows = sqlx::query_as::<_, (i64, String, String, bool)>(
            r#"
            SELECT DISTINCT f.id, f.filename, fo.alias, fo.is_buddy_only
            FROM files f
            JOIN folders fo ON f.folder_id = fo.id
            LEFT JOIN file_terms ft ON f.id = ft.file_id
            LEFT JOIN terms t ON ft.term_id = t.id
            WHERE LOWER(f.filename) LIKE ? OR t.term LIKE ?
            ORDER BY f.filename
            "#,
        )
        .bind(&search_pattern)
        .bind(&search_pattern)
        .fetch_all(&self.pool)
        .await?;

        let mut files = Vec::new();
        let mut private_files = Vec::new();

        for (file_id, filename, folder_alias, is_buddy_only) in rows {
            // Get metadata if it exists
            let (attributes, file_size) = self.get_file_metadata(file_id).await?.unwrap_or((Vec::new(), None));
            let full_path = self.alias_components_to_path(&folder_alias, &filename);

            let file = File {
                code: 0,
                filename: format!("{folder_alias}\\{filename}"),
                file_size: file_size.unwrap_or_default(),
                extension: filename
                    .rsplit_once('.')
                    .map(|(_, ext)| ext.to_string())
                    .unwrap_or_default(),
                attributes,
            };

            
            if is_buddy_only {
                private_files.push((file, full_path));
            } else {
                files.push((file, full_path));
            }
        }

        Ok((files, private_files))
    }

    fn alias_components_to_path(&self, folder_alias: &str, filename: &str) -> PathBuf {
        let (alias_root, folder) = folder_alias.split_once("\\").unwrap();
        self.alias_to_path[alias_root].join(folder).join(&filename)
    }

    /// Get metadata for a specific file
    async fn get_file_metadata(&self, file_id: i64) -> Result<Option<(Vec<FileAttribute>, Option<u64>)>, sqlx::Error> {
        let row = sqlx::query_as::<_, (Option<u32>, Option<f64>, Option<bool>, Option<u32>, Option<u32>, Option<u64>)>(
            "SELECT bitrate, duration, vbr, sample_rate, bit_depth, filesize FROM file_metadata WHERE file_id = ?"
        )
        .bind(file_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some((bitrate, duration, vbr, sample_rate, bit_depth, filesize)) = row {
            return Ok(Some((FileAttribute::from_parts(bitrate, duration.map(|d| d.round() as u32), vbr, sample_rate, bit_depth), filesize)));
        } else {
            if let Ok((filename, folder_alias)) = sqlx::query_as::<_, (String, String)>(
                "SELECT files.filename, folder.alias
                    FROM files files
                    JOIN folders folder ON files.folder_id = folder.id 
                    WHERE files.id = ?",
            )
            .bind(file_id)
            .fetch_one(&self.pool)
            .await
            {
                let path = self.alias_components_to_path(&folder_alias, &filename);
                if let Some(Ok(parsed)) = crate::parsers::parse(&path) {
                    let bitrate = *parsed.bitrate() as u32;
                    let duration = parsed.duration();
                    let vbr = parsed.is_vbr();
                    let sample_rate = parsed.sample_rate();
                    let bit_depth = parsed.bit_depth().map(|bd| bd as u32);
                    let filesize = path.metadata().map(|m| m.len()).ok();

                    let _ = self.store_file_metadata(file_id, bitrate, duration, vbr, sample_rate, bit_depth, filesize).await;

                    let mut attrs = vec![
                        FileAttribute::Bitrate(bitrate),
                        FileAttribute::Duration(duration.round() as u32),
                        FileAttribute::VBR(vbr),
                        FileAttribute::SampleRate(sample_rate),
                    ];
                    if let Some(bit_depth) = bit_depth {
                        attrs.push(FileAttribute::BitDepth(bit_depth));
                    };
                    return Ok(Some((attrs, filesize)));
                }
            };
            Ok(None)
        }
    }

    /// Store metadata for a file
    pub(crate) async fn store_file_metadata(
        &self,
        file_id: i64,
        bitrate: u32,
        duration: f64,
        vbr: bool,
        sample_rate: u32,
        bit_depth: Option<u32>,
        filesize: Option<u64>
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO file_metadata 
            (file_id, bitrate, duration, vbr, sample_rate, bit_depth, filesize)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(file_id)
        .bind(bitrate)
        .bind(duration)
        .bind(vbr)
        .bind(sample_rate)
        .bind(bit_depth)
        .bind(filesize.map(|fs| fs as i64))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get the total number of files
    pub(crate) async fn get_total_file_count(&self) -> Result<u32, sqlx::Error> {
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM files")
            .fetch_one(&self.pool)
            .await?;

        Ok(count as u32)
    }

    /// Get the number of folders that contain files
    pub(crate) async fn get_folder_count(&self) -> Result<u32, sqlx::Error> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(DISTINCT folders.id)
            FROM folders
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count as u32)
    }

    /// Index a folder and all its children recursively
    pub(crate) async fn index_folder(
        &mut self,
        folder_path: impl AsRef<std::path::Path>,
        alias: &str,
        is_buddy_only: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let folder_path = folder_path.as_ref();

        // Check if folder exists
        if !folder_path.exists() || !folder_path.is_dir() {
            return Err(format!(
                "Path does not exist or is not a directory: {:?}",
                folder_path
            )
            .into());
        }

        // Start a transaction for the entire indexing operation
        let mut tx = self.pool.begin().await?;

        // Insert or update the folder
        sqlx::query(
            r#"
            INSERT INTO root_folders (path, alias, is_buddy_only)
            VALUES (?, ?, ?)
            ON CONFLICT(path) DO UPDATE SET
                alias = excluded.alias,
                is_buddy_only = excluded.is_buddy_only
            "#,
        )
        .bind(folder_path.to_string_lossy().as_ref())
        .bind(alias)
        .bind(is_buddy_only)
        .execute(&self.pool)
        .await?;

        // Walk the directory and index all files
        let walker = walkdir::WalkDir::new(folder_path)
            .follow_links(false)
            .into_iter()
            // don't traverse hidden folders
            .filter_entry(|dir| {
                let mut components = dir.path().components().rev();
                components.next().unwrap();
                let folder = Path::new(components.next().unwrap().as_os_str());
                !file_is_hidden(folder) & !file_is_hidden(dir.path())
            });

        let mut files_to_insert = Vec::new();

        for entry in walker {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    eprintln!("Warning: Error accessing entry: {}", e);
                    continue;
                }
            };

            if entry.file_type().is_file() {
                let file_path = entry.path();
                let parent_dir = file_path.parent().unwrap_or(folder_path);

                // Only index files that are direct children or in subdirectories
                if let Some(filename) = file_path.file_name() {
                    let filename_str = filename.to_string_lossy();

                    // Get file metadata (modification time)
                    let modified_time = match entry.metadata() {
                        Ok(m) => m
                            .modified()
                            .map(|t| {
                                t.duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs() as i64
                            })
                            .unwrap_or(0),
                        Err(_) => 0,
                    };

                    // Determine which folder this file belongs to
                    let relative_parent = parent_dir
                        .strip_prefix(folder_path)
                        .unwrap_or(std::path::Path::new(""));
                    let file_folder_path = if relative_parent.as_os_str().is_empty() {
                        folder_path.to_path_buf()
                    } else {
                        folder_path.join(relative_parent)
                    };

                    files_to_insert.push((
                        file_folder_path,
                        filename_str.to_string(),
                        modified_time,
                    ));
                }
            }
        }

        // Group files by their parent folder and ensure all folders exist
        let mut folder_cache: HashMap<PathBuf, i64> = HashMap::new();

        for (file_folder_path, filename, modified_time) in files_to_insert {
            let current_folder_id = if let Some(&cached_id) = folder_cache.get(&file_folder_path) {
                cached_id
            } else {
                // Create subfolder entry if it doesn't exist
                let relative_path = file_folder_path
                    .strip_prefix(folder_path)
                    .unwrap_or(std::path::Path::new(""));
                let subfolder_alias = if relative_path.as_os_str().is_empty() {
                    alias.to_string()
                } else {
                    format!("{}\\{}", alias, relative_path.to_string_lossy())
                };

                let subfolder_id = sqlx::query_scalar::<_, i64>(
                    r#"
                    INSERT INTO folders (alias, is_buddy_only)
                    VALUES (?, ?)
                    ON CONFLICT(alias) DO UPDATE SET alias = alias
                    RETURNING id
                    "#,
                )
                .bind(&subfolder_alias)
                .bind(is_buddy_only)
                .fetch_one(&mut *tx)
                .await?;

                folder_cache.insert(file_folder_path.clone(), subfolder_id);
                subfolder_id
            };

            // Insert the file
            let file_id = match sqlx::query_scalar::<_, i64>(
                r#"
                INSERT OR IGNORE INTO files (folder_id, filename, modified_time)
                VALUES (?, ?, ?)
                RETURNING id
                "#,
            )
            .bind(current_folder_id)
            .bind(&filename)
            .bind(modified_time)
            .fetch_one(&mut *tx)
            .await
            {
                Ok(file_id) => file_id,
                Err(_) => {
                    continue;
                }
            };

            // Extract and index terms from filename
            self.index_file_terms(&mut tx, file_id, &filename).await?;
        }

        sqlx::query(
            r#"
            DELETE FROM folders
            WHERE NOT EXISTS (
                SELECT 1 FROM files 
                WHERE files.folder_id = folders.id
            )
            "#,
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        // Update in-memory mappings
        self.folder_aliases
            .insert(folder_path.to_path_buf(), alias.to_string());
        self.alias_to_path
            .insert(alias.to_string(), folder_path.to_path_buf());

        Ok(())
    }

    /// Extract terms from filename and store them for fast searching
    async fn index_file_terms(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        file_id: i64,
        filename: &str,
    ) -> Result<(), sqlx::Error> {
        // Simple term extraction - split on common delimiters and normalize
        let terms: Vec<String> = filename
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|term| !term.is_empty() && term.len() > 1)
            .map(|term| term.to_string())
            .collect();

        for term in terms {
            // Insert term if it doesn't exist
            let term_id = sqlx::query_scalar::<_, i64>(
                r#"
                INSERT INTO terms (term)
                VALUES (?)
                ON CONFLICT(term) DO UPDATE SET term = term
                RETURNING id
                "#,
            )
            .bind(&term)
            .fetch_one(&mut **tx)
            .await?;

            // Link file to term
            sqlx::query("INSERT OR IGNORE INTO file_terms (file_id, term_id) VALUES (?, ?)")
                .bind(file_id)
                .bind(term_id)
                .execute(&mut **tx)
                .await?;
        }

        Ok(())
    }

    /// Re-index all known folders (useful for updates)
    pub(crate) async fn reindex_all(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Get all folders from database
        let folders = sqlx::query_as::<_, (String, String, bool)>(
            "SELECT path, alias, is_buddy_only FROM root_folders",
        )
        .fetch_all(&self.pool)
        .await?;

        for (path, alias, is_buddy_only) in folders {
            let folder_path = Path::new(&path);
            if folder_path.exists() {
                self.index_folder(folder_path, &alias, is_buddy_only)
                    .await?;
            }
        }

        Ok(())
    }

    /// Remove a folder and all its files from the index
    pub(crate) async fn remove_folder(
        &mut self,
        alias: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut tx = self.pool.begin().await?;

        // Get the folder path before deletion
        let folder_path = self.alias_to_path.get(alias).cloned();

        // Delete the folder (CASCADE will handle files and terms)
        sqlx::query("DELETE FROM folders WHERE alias = ?")
            .bind(alias)
            .execute(&mut *tx)
            .await?;

        // Clean up orphaned terms
        sqlx::query(
            r#"
            DELETE FROM terms 
            WHERE id NOT IN (SELECT DISTINCT term_id FROM file_terms)
            "#,
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        // Update in-memory mappings
        if let Some(path) = folder_path {
            self.folder_aliases.remove(&path);
        }
        self.alias_to_path.remove(alias);

        Ok(())
    }
}
