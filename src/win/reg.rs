//! 레지스트리 문자열 값 읽기·쓰기·삭제 (자동 실행, 크롬 경로 탐색에 사용).

use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    HKEY, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};
use windows::core::{HSTRING, PCWSTR};

// HKEY_LOCAL_MACHINE은 크롬 경로 탐색(Task 6)에서 쓰기 시작한다.
#[allow(unused_imports)]
pub use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// `value`가 빈 문자열이면 키의 기본값을 읽는다.
pub fn get_string(root: HKEY, subkey: &str, value: &str) -> Option<String> {
    let subkey = HSTRING::from(subkey);
    let name = wide(value);
    let name_ptr = if value.is_empty() { PCWSTR::null() } else { PCWSTR(name.as_ptr()) };
    let mut size = 0u32;
    // SAFETY: 크기만 묻는 호출, 버퍼 없음
    let status = unsafe { RegGetValueW(root, &subkey, name_ptr, RRF_RT_REG_SZ, None, None, Some(&mut size)) };
    if status != ERROR_SUCCESS || size == 0 {
        return None;
    }
    let mut buf = vec![0u16; (size as usize).div_ceil(2)];
    // SAFETY: buf는 size 바이트 이상
    let status = unsafe {
        RegGetValueW(root, &subkey, name_ptr, RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr().cast()), Some(&mut size))
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

pub fn set_string(root: HKEY, subkey: &str, value: &str, data: &str) -> windows::core::Result<()> {
    let data = wide(data);
    // SAFETY: data는 NUL로 끝나는 UTF-16, 길이는 바이트 단위
    unsafe {
        RegSetKeyValueW(
            root,
            &HSTRING::from(subkey),
            &HSTRING::from(value),
            REG_SZ.0,
            Some(data.as_ptr().cast()),
            (data.len() * 2) as u32,
        )
    }
    .ok()
}

/// 값이 없으면 아무 일도 하지 않는다.
pub fn delete_value(root: HKEY, subkey: &str, value: &str) {
    // SAFETY: 단순 삭제 호출
    let _ = unsafe { RegDeleteKeyValueW(root, &HSTRING::from(subkey), &HSTRING::from(value)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = r"Software\YT-Launcher-Test";

    #[test]
    fn string_roundtrip_in_hkcu() {
        let name = format!("v{}", crate::items::new_id());
        set_string(HKEY_CURRENT_USER, KEY, &name, "한글 값 \"C:\\x.exe\"").unwrap();
        assert_eq!(get_string(HKEY_CURRENT_USER, KEY, &name).as_deref(), Some("한글 값 \"C:\\x.exe\""));
        delete_value(HKEY_CURRENT_USER, KEY, &name);
        assert_eq!(get_string(HKEY_CURRENT_USER, KEY, &name), None);
        delete_value(HKEY_CURRENT_USER, KEY, &name); // 없는 값 삭제도 문제없음
    }
}
