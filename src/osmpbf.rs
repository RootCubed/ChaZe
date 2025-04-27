use bimap;
use flate2::read::ZlibDecoder;
use prost::Message;
use std::collections::{BTreeMap, HashMap};
use std::io::Read;

// Protobuf description for the OpenStreetMap PBF format
mod osmpbf {
    include!(concat!(env!("OUT_DIR"), "/osmpbf.rs"));
}

pub struct OsmElement<T> {
    id: i64,
    tags: HashMap<u32, u32>,
    el: T,
}

pub struct OsmNodeData {
    pub lat: i64,
    pub lon: i64,
}
pub struct OsmWayData {
    pub refs: Vec<i64>,
}

pub struct OsmRelationMemberInfo {
    pub ref_id: i64,
    pub role_sid: u32,
}

#[derive(PartialEq, Eq)]
pub enum OsmRelationMemberType {
    Node,
    Way,
    Relation,
}

pub struct OsmRelationData {
    pub members: Vec<(OsmRelationMemberType, OsmRelationMemberInfo)>,
}

#[derive(Default)]
pub struct StringTable {
    strings: bimap::BiMap<u32, String>,
    curr_id: u32,
}

impl StringTable {
    pub fn insert(&mut self, s: String) -> u32 {
        if let Some(k) = self.strings.get_by_right(&s) {
            return *k;
        }
        let id = self.curr_id;
        self.curr_id += 1;
        self.strings.insert(id, s);
        id
    }

    pub fn lookup_idx(&self, s: &str) -> Option<u32> {
        self.strings.get_by_right(s).map(|x| *x)
    }

    pub fn get(&self, id: u32) -> Option<&String> {
        self.strings.get_by_left(&id)
    }
}

pub struct OsmFileElement<'a, T> {
    pub el: &'a OsmElement<T>,
    pub osm_file: &'a OsmFile,
}

type IDMap<T> = std::collections::HashMap<i64, T>;

#[derive(Default)]
pub struct OsmFile {
    header: osmpbf::HeaderBlock,
    string_table: StringTable,
    nodes: IDMap<OsmElement<OsmNodeData>>,
    ways: IDMap<OsmElement<OsmWayData>>,
    relations: IDMap<OsmElement<OsmRelationData>>,
}

impl OsmFile {
    pub fn get_string(&self, id: u32) -> Option<&String> {
        self.string_table.get(id)
    }

    pub fn get_string_idx(&self, s: &str) -> Option<u32> {
        self.string_table.lookup_idx(s)
    }

    pub fn nodes(&self) -> impl Iterator<Item = OsmFileElement<OsmNodeData>> {
        self.nodes
            .values()
            .map(|el| OsmFileElement { el, osm_file: self })
    }

    pub fn ways(&self) -> impl Iterator<Item = OsmFileElement<OsmWayData>> {
        self.ways
            .values()
            .map(|el| OsmFileElement { el, osm_file: self })
    }

    pub fn relations(&self) -> impl Iterator<Item = OsmFileElement<OsmRelationData>> {
        self.relations
            .values()
            .map(|el| OsmFileElement { el, osm_file: self })
    }

    pub fn get_node(&self, id: i64) -> Option<OsmFileElement<OsmNodeData>> {
        Some(OsmFileElement {
            el: self.nodes.get(&id)?,
            osm_file: self,
        })
    }

    pub fn get_way(&self, id: i64) -> Option<OsmFileElement<OsmWayData>> {
        Some(OsmFileElement {
            el: self.ways.get(&id)?,
            osm_file: self,
        })
    }

    pub fn get_relation(&self, id: i64) -> Option<OsmFileElement<OsmRelationData>> {
        Some(OsmFileElement {
            el: self.relations.get(&id)?,
            osm_file: self,
        })
    }

    #[allow(dead_code)]
    pub fn get_el_name(&self, id: i64) -> String {
        if let Some(node) = self.get_node(id) {
            return node
                .get_tag_value("name")
                .unwrap_or(&"".to_string())
                .clone();
        }
        if let Some(way) = self.get_way(id) {
            return way.get_tag_value("name").unwrap_or(&"".to_string()).clone();
        }
        if let Some(rel) = self.get_relation(id) {
            return rel.get_tag_value("name").unwrap_or(&"".to_string()).clone();
        }
        return "".to_string();
    }
}

impl<T> OsmFileElement<'_, T> {
    pub fn id(&self) -> i64 {
        self.el.id
    }

    pub fn data(&self) -> &T {
        &self.el.el
    }

    #[allow(dead_code)]
    pub fn tags(&self) -> BTreeMap<&String, &String> {
        let mut res = BTreeMap::new();
        for (k_id, v_id) in &self.el.tags {
            let k = self.osm_file.get_string(*k_id).unwrap();
            let v = self.osm_file.get_string(*v_id).unwrap();
            res.insert(k, v);
        }
        res
    }

    pub fn get_tag_value(&self, k: &str) -> Option<&String> {
        let k_id = self.osm_file.get_string_idx(k)?;
        let v_id = self.el.tags.get(&k_id)?;
        self.osm_file.get_string(*v_id)
    }
}

fn decompress_blob_data(blob: osmpbf::Blob) -> Result<Vec<u8>, std::io::Error> {
    match blob.data {
        Some(osmpbf::blob::Data::ZlibData(d)) => {
            let mut blob_data = Vec::new();
            ZlibDecoder::new(&d[..])
                .read_to_end(&mut blob_data)
                .unwrap();
            Ok(blob_data)
        }
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Unknown blob data type",
        )),
    }
}

fn tag_to_string_table_idxs(
    k: u32,
    v: u32,
    osm_string_table: &[String],
    string_table: &mut StringTable,
) -> (u32, u32) {
    let k_str = osm_string_table[k as usize].clone();
    let v_str = osm_string_table[v as usize].clone();
    let k_id = string_table.insert(k_str);
    let v_id = string_table.insert(v_str);
    (k_id, v_id)
}

fn read_dense_tags(
    osm_file: &mut OsmFile,
    string_table: &[String],
    keys_vals: &[i32],
) -> Vec<HashMap<u32, u32>> {
    let mut tags = HashMap::new();
    let mut res = Vec::new();
    let mut i = 0;
    while i < keys_vals.len() {
        let k = keys_vals[i];
        if k == 0 {
            res.push(tags);
            tags = HashMap::new();
            i += 1;
            continue;
        }
        let v = keys_vals[i + 1];
        let (k_id, v_id) =
            tag_to_string_table_idxs(k as u32, v as u32, string_table, &mut osm_file.string_table);
        tags.insert(k_id, v_id);
        i += 2;
    }
    res
}

pub fn read_osm_nodes(osm_file: &mut OsmFile, string_table: &[String], nodes: &[osmpbf::Node]) {
    for node in nodes {
        let mut tags = HashMap::new();
        for (k, v) in node.keys.iter().zip(node.vals.iter()) {
            let (k_id, v_id) =
                tag_to_string_table_idxs(*k, *v, string_table, &mut osm_file.string_table);
            tags.insert(k_id, v_id);
        }
        osm_file.nodes.insert(
            node.id,
            OsmElement {
                id: node.id,
                tags,
                el: OsmNodeData {
                    lat: node.lat,
                    lon: node.lon,
                },
            },
        );
    }
}

pub fn read_osm_dense_nodes(
    osm_file: &mut OsmFile,
    string_table: &[String],
    dense_nodes: &osmpbf::DenseNodes,
) {
    let mut id = 0;
    let mut lat = 0;
    let mut lon = 0;
    let tags = read_dense_tags(osm_file, string_table, &dense_nodes.keys_vals);
    osm_file.nodes.reserve(tags.len());
    for (i, t) in tags.into_iter().enumerate() {
        id += dense_nodes.id[i];
        lat += dense_nodes.lat[i];
        lon += dense_nodes.lon[i];
        osm_file.nodes.insert(
            id,
            OsmElement {
                id,
                tags: t,
                el: OsmNodeData { lat, lon },
            },
        );
    }
}

pub fn read_osm_ways(osm_file: &mut OsmFile, string_table: &[String], ways: &[osmpbf::Way]) {
    for way in ways {
        let mut tags = HashMap::new();
        for (k, v) in way.keys.iter().zip(way.vals.iter()) {
            let (k_id, v_id) =
                tag_to_string_table_idxs(*k, *v, string_table, &mut osm_file.string_table);
            tags.insert(k_id, v_id);
        }
        let refs = way
            .refs
            .iter()
            .scan(0, |state, &x| {
                *state += x;
                Some(*state)
            })
            .collect();
        osm_file.ways.insert(
            way.id,
            OsmElement {
                id: way.id,
                tags,
                el: OsmWayData { refs },
            },
        );
    }
}

pub fn read_osm_relations(
    osm_file: &mut OsmFile,
    string_table: &[String],
    relations: &[osmpbf::Relation],
) {
    for relation in relations {
        let mut tags = HashMap::new();
        for (k, v) in relation.keys.iter().zip(relation.vals.iter()) {
            let (k_id, v_id) =
                tag_to_string_table_idxs(*k, *v, string_table, &mut osm_file.string_table);
            tags.insert(k_id, v_id);
        }
        let mut members = Vec::new();
        let mut id: i64 = 0;
        for (i, mem_id) in relation.memids.iter().enumerate() {
            id += *mem_id;
            let role_orig_sid = relation.roles_sid[i] as usize;
            let role_sid = osm_file
                .string_table
                .insert(string_table[role_orig_sid].clone());
            let member = (
                match osmpbf::relation::MemberType::try_from(relation.types[i]) {
                    Ok(osmpbf::relation::MemberType::Node) => OsmRelationMemberType::Node,
                    Ok(osmpbf::relation::MemberType::Way) => OsmRelationMemberType::Way,
                    Ok(osmpbf::relation::MemberType::Relation) => OsmRelationMemberType::Relation,
                    Err(_) => panic!("Unknown member type"),
                },
                OsmRelationMemberInfo {
                    ref_id: id,
                    role_sid,
                },
            );
            members.push(member);
        }
        osm_file.relations.insert(
            relation.id,
            OsmElement {
                id: relation.id,
                tags,
                el: OsmRelationData { members },
            },
        );
    }
}

pub fn read_osm_data(blob: osmpbf::PrimitiveBlock, osm_file: &mut OsmFile) {
    let blob_stringtable = blob
        .stringtable
        .s
        .into_iter()
        .map(|s| String::from_utf8(s).unwrap())
        .collect::<Vec<String>>();
    for group in blob.primitivegroup {
        if group.nodes.len() > 0 {
            read_osm_nodes(osm_file, &blob_stringtable, &group.nodes);
        }
        if let Some(dense_nodes) = group.dense {
            read_osm_dense_nodes(osm_file, &blob_stringtable, &dense_nodes);
        }
        if group.ways.len() > 0 {
            read_osm_ways(osm_file, &blob_stringtable, &group.ways);
        }
        if group.relations.len() > 0 {
            read_osm_relations(osm_file, &blob_stringtable, &group.relations);
        }
    }
}

pub fn read_osm_blob(bytes: &[u8], osm_file: &mut OsmFile) -> Result<usize, std::io::Error> {
    let header_size = u32::from_be_bytes(bytes[0..4].try_into().unwrap()) as usize;
    let header = osmpbf::BlobHeader::decode(&bytes[4..4 + header_size]).unwrap();
    let spos = 4 + header_size as usize;
    let data = osmpbf::Blob::decode(&bytes[spos..spos + header.datasize as usize])?;
    let blob_data = decompress_blob_data(data)?;

    if header.r#type == "OSMHeader" {
        osm_file.header = osmpbf::HeaderBlock::decode(blob_data.as_slice())?;
    } else if header.r#type == "OSMData" {
        let blob = osmpbf::PrimitiveBlock::decode(blob_data.as_slice())?;
        read_osm_data(blob, osm_file);
    } else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Unknown blob type",
        ));
    }
    Ok(spos + header.datasize as usize)
}

pub fn read_osm_file(file: &[u8]) -> Result<OsmFile, std::io::Error> {
    let mut f = OsmFile::default();
    let mut pos = 0;
    while pos < file.len() {
        match read_osm_blob(&file[pos..], &mut f) {
            Ok(n) => pos += n,
            Err(e) => return Err(e),
        }
    }
    Ok(f)
}
