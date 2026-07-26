//! Magic-byte content-type detection (Phase 7.3).
//!
//! Hand-rolled, curated signature table — not a full `file`-command replacement.
//! libmagic was considered and rejected: it's not bundled on Windows, which breaks
//! rwf's "no install step" cross-platform goal (see plan/7.3.smart_file_opener.md).

/// A content type identified by inspecting a file's leading bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedKind {
    Png,
    Jpeg,
    Gif,
    Bmp,
    WebP,
    Zip,
    Gzip,
    SevenZ,
    Pdf,
    Pe,
    Elf,
    MachO,
    /// No signature in the table matched — not an error, just "couldn't tell."
    Unknown,
}

impl DetectedKind {
    /// Human-readable label for dialogs/logs.
    pub fn label(self) -> &'static str {
        match self {
            DetectedKind::Png => "PNG image",
            DetectedKind::Jpeg => "JPEG image",
            DetectedKind::Gif => "GIF image",
            DetectedKind::Bmp => "BMP image",
            DetectedKind::WebP => "WebP image",
            DetectedKind::Zip => "ZIP archive",
            DetectedKind::Gzip => "GZIP archive",
            DetectedKind::SevenZ => "7-Zip archive",
            DetectedKind::Pdf => "PDF document",
            DetectedKind::Pe => "Windows PE executable",
            DetectedKind::Elf => "ELF executable",
            DetectedKind::MachO => "Mach-O executable",
            DetectedKind::Unknown => "Unknown",
        }
    }

    /// True for the three executable kinds — the only kinds `is_mismatch` ever
    /// flags (see module doc comment and the `mismatch_never_fires_for_non_executable_kinds`
    /// test for why images/archives/PDF are excluded).
    pub fn is_executable(self) -> bool {
        matches!(
            self,
            DetectedKind::Pe | DetectedKind::Elf | DetectedKind::MachO
        )
    }

    /// Extensions (lowercase, no dot) that legitimately carry this content type.
    /// Only meaningful for executable kinds — `is_mismatch` is the only caller.
    fn expected_extensions(self) -> &'static [&'static str] {
        match self {
            DetectedKind::Pe => &["exe", "dll", "com", "scr", "sys", "cpl"],
            DetectedKind::Elf => &["so", "o"],
            DetectedKind::MachO => &["dylib", "bundle"],
            _ => &[],
        }
    }

    /// Stable short config key for this kind, as written in an
    /// `ExtensionAssociation.FileType` field (Phase 7.3b). `Unknown` has no key —
    /// it never matches any `FileType` spec (see `matches_file_type_spec`).
    pub fn config_key(self) -> Option<&'static str> {
        match self {
            DetectedKind::Png => Some("png"),
            DetectedKind::Jpeg => Some("jpeg"),
            DetectedKind::Gif => Some("gif"),
            DetectedKind::Bmp => Some("bmp"),
            DetectedKind::WebP => Some("webp"),
            DetectedKind::Zip => Some("zip"),
            DetectedKind::Gzip => Some("gzip"),
            DetectedKind::SevenZ => Some("7z"),
            DetectedKind::Pdf => Some("pdf"),
            DetectedKind::Pe => Some("pe"),
            DetectedKind::Elf => Some("elf"),
            DetectedKind::MachO => Some("macho"),
            DetectedKind::Unknown => None,
        }
    }

    /// True if `spec` (a `FileType` value from `extension_associations.json`,
    /// case-insensitive) matches this kind — either the exact `config_key`, or one
    /// of the three group aliases: `"image"` (png/jpeg/gif/bmp/webp), `"archive"`
    /// (zip/gzip/7z), `"executable"` (pe/elf/macho). `Unknown` never matches
    /// anything, including an unrecognized `spec` string (Phase 7.3b).
    pub fn matches_file_type_spec(self, spec: &str) -> bool {
        if self == DetectedKind::Unknown {
            return false;
        }
        let spec_lower = spec.to_lowercase();
        if Some(spec_lower.as_str()) == self.config_key() {
            return true;
        }
        match spec_lower.as_str() {
            "image" => matches!(
                self,
                DetectedKind::Png
                    | DetectedKind::Jpeg
                    | DetectedKind::Gif
                    | DetectedKind::Bmp
                    | DetectedKind::WebP
            ),
            "archive" => matches!(
                self,
                DetectedKind::Zip | DetectedKind::Gzip | DetectedKind::SevenZ
            ),
            "executable" => self.is_executable(),
            _ => false,
        }
    }
}

/// Identify a file's content type from its leading bytes. `bytes` should be the
/// first ~300 bytes of the file (see `JobKind::DetectFileType`); shorter input
/// just means more signatures fail to match and the result leans `Unknown`.
pub fn detect_kind(bytes: &[u8]) -> DetectedKind {
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return DetectedKind::Png;
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return DetectedKind::Jpeg;
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return DetectedKind::Gif;
    }
    if bytes.starts_with(b"BM") {
        return DetectedKind::Bmp;
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return DetectedKind::WebP;
    }
    if bytes.starts_with(&[0x50, 0x4B, 0x03, 0x04]) {
        return DetectedKind::Zip;
    }
    if bytes.starts_with(&[0x1F, 0x8B]) {
        return DetectedKind::Gzip;
    }
    if bytes.starts_with(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]) {
        return DetectedKind::SevenZ;
    }
    if bytes.starts_with(b"%PDF-") {
        return DetectedKind::Pdf;
    }
    if bytes.starts_with(&[0x4D, 0x5A]) {
        return DetectedKind::Pe;
    }
    if bytes.starts_with(&[0x7F, 0x45, 0x4C, 0x46]) {
        return DetectedKind::Elf;
    }
    if bytes.starts_with(&[0xFE, 0xED, 0xFA, 0xCE])
        || bytes.starts_with(&[0xFE, 0xED, 0xFA, 0xCF])
        || bytes.starts_with(&[0xCE, 0xFA, 0xED, 0xFE])
        || bytes.starts_with(&[0xCF, 0xFA, 0xED, 0xFE])
    {
        return DetectedKind::MachO;
    }
    DetectedKind::Unknown
}

/// True if `kind` disagrees enough with `extension` to be worth warning about
/// before running an `ExtensionAssociation` command. Only executable kinds ever
/// return true (see `DetectedKind::is_executable`).
pub fn is_mismatch(extension: &str, kind: DetectedKind) -> bool {
    if !kind.is_executable() {
        return false;
    }
    let ext_lower = extension.to_lowercase();
    !kind.expected_extensions().iter().any(|e| *e == ext_lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_png() {
        let bytes = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00];
        assert_eq!(detect_kind(&bytes), DetectedKind::Png);
    }

    #[test]
    fn detects_jpeg() {
        let bytes = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        assert_eq!(detect_kind(&bytes), DetectedKind::Jpeg);
    }

    #[test]
    fn detects_gif() {
        assert_eq!(detect_kind(b"GIF89a\x01\x00"), DetectedKind::Gif);
        assert_eq!(detect_kind(b"GIF87a\x01\x00"), DetectedKind::Gif);
    }

    #[test]
    fn detects_bmp() {
        assert_eq!(detect_kind(b"BM\x46\x00\x00\x00"), DetectedKind::Bmp);
    }

    #[test]
    fn detects_webp() {
        let mut bytes = b"RIFF".to_vec();
        bytes.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // chunk size, irrelevant
        bytes.extend_from_slice(b"WEBP");
        assert_eq!(detect_kind(&bytes), DetectedKind::WebP);
    }

    #[test]
    fn detects_zip() {
        assert_eq!(
            detect_kind(&[0x50, 0x4B, 0x03, 0x04, 0x14, 0x00]),
            DetectedKind::Zip
        );
    }

    #[test]
    fn detects_gzip() {
        assert_eq!(detect_kind(&[0x1F, 0x8B, 0x08, 0x00]), DetectedKind::Gzip);
    }

    #[test]
    fn detects_sevenz() {
        assert_eq!(
            detect_kind(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C, 0x00, 0x04]),
            DetectedKind::SevenZ
        );
    }

    #[test]
    fn detects_pdf() {
        assert_eq!(detect_kind(b"%PDF-1.7\n"), DetectedKind::Pdf);
    }

    #[test]
    fn detects_pe() {
        assert_eq!(detect_kind(&[0x4D, 0x5A, 0x90, 0x00]), DetectedKind::Pe);
    }

    #[test]
    fn detects_elf() {
        assert_eq!(
            detect_kind(&[0x7F, 0x45, 0x4C, 0x46, 0x02, 0x01]),
            DetectedKind::Elf
        );
    }

    #[test]
    fn detects_macho() {
        assert_eq!(detect_kind(&[0xFE, 0xED, 0xFA, 0xCE]), DetectedKind::MachO);
        assert_eq!(detect_kind(&[0xCF, 0xFA, 0xED, 0xFE]), DetectedKind::MachO);
    }

    #[test]
    fn unknown_for_plain_text() {
        assert_eq!(detect_kind(b"hello, world\n"), DetectedKind::Unknown);
    }

    #[test]
    fn unknown_for_empty_input() {
        assert_eq!(detect_kind(&[]), DetectedKind::Unknown);
    }

    #[test]
    fn label_is_human_readable() {
        assert_eq!(DetectedKind::Png.label(), "PNG image");
        assert_eq!(DetectedKind::Pe.label(), "Windows PE executable");
        assert_eq!(DetectedKind::Unknown.label(), "Unknown");
    }

    #[test]
    fn mismatch_never_fires_for_non_executable_kinds() {
        // Images/archives/PDF have too many legitimate extension aliases (jpg/jpeg,
        // zip-based container formats like .docx) to flag reliably — v1 only warns
        // on executables, the "実害のある" (harmful) case per the design doc.
        assert!(!is_mismatch("txt", DetectedKind::Png));
        assert!(!is_mismatch("docx", DetectedKind::Zip));
        assert!(!is_mismatch("anything", DetectedKind::Pdf));
    }

    #[test]
    fn mismatch_fires_for_executable_with_unexpected_extension() {
        assert!(is_mismatch("txt", DetectedKind::Pe));
        assert!(is_mismatch("docx", DetectedKind::Elf));
    }

    #[test]
    fn mismatch_does_not_fire_for_executable_with_expected_extension() {
        assert!(!is_mismatch("exe", DetectedKind::Pe));
        assert!(!is_mismatch("EXE", DetectedKind::Pe)); // case-insensitive
        assert!(!is_mismatch("dll", DetectedKind::Pe));
    }

    #[test]
    fn mismatch_never_fires_for_unknown() {
        assert!(!is_mismatch("txt", DetectedKind::Unknown));
    }

    const ALL_KINDS: &[DetectedKind] = &[
        DetectedKind::Png,
        DetectedKind::Jpeg,
        DetectedKind::Gif,
        DetectedKind::Bmp,
        DetectedKind::WebP,
        DetectedKind::Zip,
        DetectedKind::Gzip,
        DetectedKind::SevenZ,
        DetectedKind::Pdf,
        DetectedKind::Pe,
        DetectedKind::Elf,
        DetectedKind::MachO,
        DetectedKind::Unknown,
    ];

    #[test]
    fn config_key_table() {
        let cases: &[(DetectedKind, Option<&str>)] = &[
            (DetectedKind::Png, Some("png")),
            (DetectedKind::Jpeg, Some("jpeg")),
            (DetectedKind::Gif, Some("gif")),
            (DetectedKind::Bmp, Some("bmp")),
            (DetectedKind::WebP, Some("webp")),
            (DetectedKind::Zip, Some("zip")),
            (DetectedKind::Gzip, Some("gzip")),
            (DetectedKind::SevenZ, Some("7z")),
            (DetectedKind::Pdf, Some("pdf")),
            (DetectedKind::Pe, Some("pe")),
            (DetectedKind::Elf, Some("elf")),
            (DetectedKind::MachO, Some("macho")),
            (DetectedKind::Unknown, None),
        ];
        for (kind, expected) in cases {
            assert_eq!(kind.config_key(), *expected, "config_key for {:?}", kind);
        }
        // Every non-Unknown kind above is covered; keep this in sync with ALL_KINDS.
        assert_eq!(cases.len(), ALL_KINDS.len());
    }

    #[test]
    fn matches_file_type_spec_exact_key_per_kind() {
        for kind in ALL_KINDS {
            if let Some(key) = kind.config_key() {
                assert!(
                    kind.matches_file_type_spec(key),
                    "{:?} should match its own config_key {:?}",
                    kind,
                    key
                );
                // Case-insensitive
                assert!(kind.matches_file_type_spec(&key.to_uppercase()));
            }
        }
    }

    #[test]
    fn matches_file_type_spec_group_aliases() {
        let images = [
            DetectedKind::Png,
            DetectedKind::Jpeg,
            DetectedKind::Gif,
            DetectedKind::Bmp,
            DetectedKind::WebP,
        ];
        let archives = [DetectedKind::Zip, DetectedKind::Gzip, DetectedKind::SevenZ];
        let executables = [DetectedKind::Pe, DetectedKind::Elf, DetectedKind::MachO];

        for kind in ALL_KINDS {
            let expect_image = images.contains(kind);
            let expect_archive = archives.contains(kind);
            let expect_executable = executables.contains(kind);
            assert_eq!(
                kind.matches_file_type_spec("image"),
                expect_image,
                "image alias for {:?}",
                kind
            );
            assert_eq!(
                kind.matches_file_type_spec("ARCHIVE"),
                expect_archive,
                "archive alias (case-insensitive) for {:?}",
                kind
            );
            assert_eq!(
                kind.matches_file_type_spec("executable"),
                expect_executable,
                "executable alias for {:?}",
                kind
            );
        }
    }

    #[test]
    fn matches_file_type_spec_unknown_never_matches() {
        assert!(!DetectedKind::Unknown.matches_file_type_spec("image"));
        assert!(!DetectedKind::Unknown.matches_file_type_spec("archive"));
        assert!(!DetectedKind::Unknown.matches_file_type_spec("executable"));
        assert!(!DetectedKind::Unknown.matches_file_type_spec("png"));
        assert!(!DetectedKind::Unknown.matches_file_type_spec("anything"));
    }

    #[test]
    fn matches_file_type_spec_unrecognized_spec_matches_nothing() {
        for kind in ALL_KINDS {
            assert!(!kind.matches_file_type_spec("not-a-real-spec"));
        }
    }
}
