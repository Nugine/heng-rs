use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum File {
    #[serde(rename = "url")]
    Url {
        url: String,
        hashsum: Option<String>,
    },
    #[serde(rename = "direct")]
    Direct {
        content: String,
        hashsum: Option<String>,
        base64: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DynamicFile {
    BuiltIn { name: String },
    Remote { name: String, file: File },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestPolicy {
    #[serde(rename = "fuse")]
    Fuse,
    #[serde(rename = "all")]
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub input: String,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Test {
    pub cases: Vec<TestCase>,
    pub policy: TestPolicy,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Environment {
    pub language: String,
    pub system: String,
    pub arch: String,
    pub options: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLimit {
    pub memory: u64,
    pub cpu_time: u64,
    pub output: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilerLimit {
    pub memory: u64,
    pub cpu_time: u64,
    pub output: u64,
    pub message: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Limit {
    pub runtime: RuntimeLimit,
    pub compiler: CompilerLimit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Executable {
    pub source: File,
    pub environment: Environment,
    pub limit: Limit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Judge {
    #[serde(rename = "normal")]
    Normal { user: Executable },
    #[serde(rename = "special")]
    Special { user: Executable, spj: Executable },
    #[serde(rename = "interactive")]
    Interactive {
        user: Executable,
        interactor: Executable,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JudgeStatus {
    pub pending: u32,
    pub preparing: u32,
    pub judging: u32,
    pub finished: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CpuHardwareStatus {
    pub percentage: u8,
    pub loadavg: Option<[u8; 3]>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryHardwareStatus {
    pub percentage: u8,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HarewareStatus {
    pub cpu: CpuHardwareStatus,
    pub memory: MemoryHardwareStatus,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JudgeState {
    Confirmed,
    Pending,
    Preparing,
    Judging,
    Finished,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum JudgeResultKind {
    Accepted,
    WrongAnswer,

    RuntimeError,
    TimeLimitExceeded,
    MemoryLimitExceeded,
    OutputLimitExceeded,

    CompileError,
    CompileTimeLimitExceeded,
    CompileMemoryLimitExceeded,
    CompileFileLimitExceeded,

    SystemError,
    SystemTimeLimitExceeded,
    SystemMemoryLimitExceeded,
    SystemOutputLimitExceeded,
    SystemRuntimeError,
    SystemCompileError,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgeCaseResult {
    pub kind: JudgeResultKind,
    pub time: u64,
    pub memory: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JudgeResult {
    pub cases: Vec<JudgeCaseResult>,
    pub extra: Option<JudgeResultExtra>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JudgeResultExtra {
    pub user: Option<ExecutionInfo>,
    pub spj: Option<ExecutionInfo>,
    pub interactive: Option<ExecutionInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionInfo {
    pub compile_message: Option<String>,
}
