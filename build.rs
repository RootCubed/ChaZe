fn main() {
    lalrpop::process_src().unwrap();
    prost_build::compile_protos(
        &["src/proto/fileformat.proto", "src/proto/osmformat.proto"],
        &["src/"],
    )
    .unwrap();
}
