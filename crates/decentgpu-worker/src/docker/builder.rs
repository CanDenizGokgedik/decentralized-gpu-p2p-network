//! Dockerfile generation for job containers.

use anyhow::Result;

/// Generate a minimal Dockerfile for a Python job.
///
/// The user's code is expected to be at `/workspace/code.py` inside the
/// image or bind-mounted via the tar provided by the hirer.
pub fn generate_python_dockerfile(requirements: bool) -> String {
    let req_line = if requirements {
        "RUN pip install --no-cache-dir -r /workspace/requirements.txt\n"
    } else {
        ""
    };

    format!(
        r#"FROM python:3.11-slim
WORKDIR /workspace
COPY . /workspace/
{req_line}CMD ["python", "/workspace/code.py"]
"#,
    )
}

/// Write a Dockerfile to a directory.
pub fn write_dockerfile(dir: &std::path::Path, content: &str) -> Result<()> {
    std::fs::write(dir.join("Dockerfile"), content)?;
    Ok(())
}
