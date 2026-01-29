use account_manager::MinecraftSession;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct MemorySettings {
    pub min_megabytes: u32,
    pub max_megabytes: u32,
}

#[derive(Debug, Clone)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub enum LaunchAuth {
    Offline {
        username: String,
        uuid: String,
    },
    Microsoft {
        username: String,
        uuid: String,
        access_token: String,
    },
}

impl LaunchAuth {
    pub fn username(&self) -> &str {
        match self {
            LaunchAuth::Offline { username, .. } => username,
            LaunchAuth::Microsoft { username, .. } => username,
        }
    }

    pub fn uuid(&self) -> &str {
        match self {
            LaunchAuth::Offline { uuid, .. } => uuid,
            LaunchAuth::Microsoft { uuid, .. } => uuid,
        }
    }

    pub fn access_token(&self) -> &str {
        match self {
            LaunchAuth::Offline { .. } => "offline-token",
            LaunchAuth::Microsoft { access_token, .. } => access_token,
        }
    }

    pub fn user_type(&self) -> &'static str {
        match self {
            LaunchAuth::Offline { .. } => "offline",
            LaunchAuth::Microsoft { .. } => "msa",
        }
    }
}

impl From<&MinecraftSession> for LaunchAuth {
    fn from(session: &MinecraftSession) -> Self {
        LaunchAuth::Microsoft {
            username: session.profile.name.clone(),
            uuid: session.profile.id.clone(),
            access_token: session.access_token.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VanillaLaunchConfig {
    pub java_path: PathBuf,
    pub game_dir: PathBuf,
    pub assets_dir: PathBuf,
    pub classpath: Vec<PathBuf>,
    pub main_class: String,
    pub version_name: String,
    pub asset_index: Option<String>,
    pub resolution: Option<Resolution>,
    pub memory: Option<MemorySettings>,
    pub extra_jvm_args: Vec<String>,
    pub extra_game_args: Vec<String>,
    pub natives_dir: Option<PathBuf>,
}

impl VanillaLaunchConfig {
    pub fn build_command(&self, auth: &LaunchAuth) -> Command {
        let mut cmd = Command::new(&self.java_path);
        cmd.current_dir(&self.game_dir);

        if let Some(memory) = &self.memory {
            cmd.arg(format!("-Xms{}M", memory.min_megabytes))
                .arg(format!("-Xmx{}M", memory.max_megabytes));
        }

        if let Some(natives) = &self.natives_dir {
            cmd.arg(format!("-Djava.library.path={}", natives.to_string_lossy()));
        }

        if !self.classpath.is_empty() {
            let classpath = self
                .classpath
                .iter()
                .map(|p| p.to_string_lossy())
                .collect::<Vec<_>>()
                .join(classpath_separator());
            cmd.arg("-cp").arg(classpath);
        }

        cmd.args(&self.extra_jvm_args);
        cmd.arg(&self.main_class);

        cmd.arg("--username").arg(auth.username());
        cmd.arg("--version").arg(&self.version_name);
        cmd.arg("--gameDir").arg(&self.game_dir);
        cmd.arg("--assetsDir").arg(&self.assets_dir);

        if let Some(asset_index) = &self.asset_index {
            cmd.arg("--assetIndex").arg(asset_index);
        }

        cmd.arg("--uuid").arg(auth.uuid());
        cmd.arg("--accessToken").arg(auth.access_token());
        
        // Legacy support (1.6.4 and older)
        // Format often expected: token:<access_token>:<uuid>
        // Or just the token. Let's try Generic legacy format.
        let session_str = format!("token:{}:{}", auth.access_token(), auth.uuid());
        cmd.arg("--session").arg(session_str);
        cmd.arg("--userType").arg(auth.user_type());
        cmd.arg("--versionType").arg("release");
        cmd.arg("--userProperties").arg("{}");

        if let Some(resolution) = &self.resolution {
            cmd.arg("--width")
                .arg(resolution.width.to_string())
                .arg("--height")
                .arg(resolution.height.to_string());
        }

        cmd.args(&self.extra_game_args);

        cmd
    }
}

fn classpath_separator() -> &'static str {
    if cfg!(windows) { ";" } else { ":" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_command_with_basic_args() {
        let cfg = VanillaLaunchConfig {
            java_path: PathBuf::from("java"),
            game_dir: PathBuf::from("/tmp/game"),
            assets_dir: PathBuf::from("/tmp/assets"),
            classpath: vec![PathBuf::from("a.jar"), PathBuf::from("b.jar")],
            main_class: "net.minecraft.client.main.Main".to_string(),
            version_name: "1.20.4".to_string(),
            asset_index: Some("1.20".to_string()),
            resolution: Some(Resolution {
                width: 854,
                height: 480,
            }),
            memory: Some(MemorySettings {
                min_megabytes: 512,
                max_megabytes: 2048,
            }),
            extra_jvm_args: vec!["-Dfile.encoding=UTF-8".to_string()],
            extra_game_args: vec!["--demo".to_string()],
            natives_dir: Some(PathBuf::from("/tmp/natives")),
        };

        let auth = LaunchAuth::Offline {
            username: "Player".into(),
            uuid: "offline-uuid".into(),
        };

        let cmd = cfg.build_command(&auth);
        let args = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert!(args.contains(&"-Xms512M".to_string()));
        assert!(args.contains(&"-Xmx2048M".to_string()));
        assert!(args.contains(&"-Djava.library.path=/tmp/natives".to_string()));
        assert!(args.contains(&"-cp".to_string()));
        assert!(args.contains(&"net.minecraft.client.main.Main".to_string()));
        assert!(args.contains(&"--username".to_string()));
        assert!(args.contains(&"--uuid".to_string()));
        assert!(args.contains(&"--accessToken".to_string()));
    }
}
