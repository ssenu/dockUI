//! exe 리소스: 아이콘(1번), 매니페스트(DPI 인식·공용 컨트롤 v6), 버전 정보(알림·파일 속성에 보이는 이름 "Dock UI").
//! 버전 숫자는 Cargo.toml 버전에서 가져온다 (릴리스 워크플로가 태그 버전을 반영한 뒤 빌드한다).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=assets/app.manifest");
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap().replace('\\', "/");
    let num = |k: &str| std::env::var(k).unwrap().parse::<u32>().unwrap_or(0);
    let (major, minor, patch) =
        (num("CARGO_PKG_VERSION_MAJOR"), num("CARGO_PKG_VERSION_MINOR"), num("CARGO_PKG_VERSION_PATCH"));
    let version = format!("{major}.{minor}.{patch}");
    let rc = format!(
        r#"#pragma code_page(65001)
1 ICON "{dir}/assets/icon.ico"
1 24 "{dir}/assets/app.manifest"
1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "041204b0"
    BEGIN
      VALUE "CompanyName", "ssenu"
      VALUE "FileDescription", "Dock UI"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "YTLauncher"
      VALUE "OriginalFilename", "YTLauncher.exe"
      VALUE "ProductName", "Dock UI"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x0412, 1200
  END
END
"#
    );
    let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("app.rc");
    std::fs::write(&out, rc).unwrap();
    embed_resource::compile(&out, embed_resource::NONE).manifest_required().unwrap();
}
