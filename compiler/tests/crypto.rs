use std::process::Command;

use tokit_compiler::{check, native, run};

fn native_output(source: &str) -> String {
    let binary = std::env::temp_dir().join(format!(
        "tokit-crypto-{}-{}{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        std::env::consts::EXE_SUFFIX
    ));
    native::build(&check(source).unwrap(), source, &binary).unwrap();
    let output = Command::new(&binary).output().unwrap();
    std::fs::remove_file(binary).unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// Published test vectors: FIPS 180-2 SHA-256 and SHA-1, RFC 1321 MD5, RFC 4231
/// HMAC (case 2), RFC 7914 PBKDF2-HMAC-SHA256, and the RFC 6455 handshake key.
const VECTORS: &str = r#"b(s:String)->Bytes{utf8_encode(s)}
main()->[String]{[hex(sha256(b("abc"))),hex(sha256(b(""))),hex(sha256(b("abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"))),hex(md5(b(""))),hex(md5(b("abc"))),hex(hmac_sha256(b("Jefe"),b("what do ya want for nothing?"))),hex(pbkdf2_sha256(b("password"),b("salt"),1)),hex(pbkdf2_sha256(b("password"),b("salt"),4096)),base64_encode(b("hello")),base64_encode(b("ab")),base64_encode(b("")),match base64_decode("aGVsbG8="){Some(d)=>match utf8_decode_bytes(d){Some(t)=>t,None=>"?"},None=>"bad"},match base64_decode("aGVsbG8"){Some(d)=>"decoded",None=>"rejected"},String(len(random_bytes(20))),hex(sha1(b("abc"))),hex(sha1(b(""))),hex(sha1(b("abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"))),base64_encode(sha1(b("dGhlIHNhbXBsZSBub25jZQ==258EAFA5-E914-47DA-95CA-C5AB0DC85B11")))]}"#;

const EXPECTED: &str = r#"["ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad","e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1","d41d8cd98f00b204e9800998ecf8427e","900150983cd24fb0d6963f7d28e17f72","5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843","120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b","c5e478d59288c841aa530db6845c4c8d962893a001ce4e11a4963873aa98134a","aGVsbG8=","YWI=","","hello","rejected","20","a9993e364706816aba3e25717850c26c9cd0d89d","da39a3ee5e6b4b0d3255bfef95601890afd80709","84983e441c3bd26ebaae4aa1f95129e5e54670f1","s3pPLMBiTxaQ9kYGzzhZRbK+xOo="]"#;

#[test]
fn hashes_and_encodings_match_published_vectors_in_both_backends() {
    assert_eq!(run(VECTORS).unwrap().to_string(), EXPECTED);
    if Command::new("rustc").arg("--version").output().is_ok() {
        assert_eq!(native_output(VECTORS), EXPECTED);
    }
}

#[test]
fn random_bytes_differ_and_are_an_effect() {
    let source = "main()->bool{random_bytes(16)==random_bytes(16)}";
    assert_eq!(run(source).unwrap().to_string(), "false");
    assert_eq!(
        check("f()->Bytes{random_bytes(4)} main()->I{let t=spawn f();1}")
            .unwrap_err()
            .code,
        "E117"
    );
    assert!(check("f()->String{hex(sha256(utf8_encode(\"x\")))} main()->Result<String,TaskError>{join(spawn f())}").is_ok());
}
