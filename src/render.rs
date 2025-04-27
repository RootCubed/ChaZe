use crate::{
    osmpbf,
    style_file::{
        self,
        ast::{self, Command, Filter, FilterExpr, FilterType},
    },
};
use rand::Rng;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Debug)]
enum OSMElementType {
    Node,
    Way,
    Relation,
}

pub struct Renderer<'a> {
    meta: &'a ast::Meta,
    cr: &'a cairo::Context,
    osm_file: &'a osmpbf::OsmFile,
    unit_scale: f64,
    rng: rand::rngs::ThreadRng,
}

#[derive(Clone)]
pub struct OSMPaintObj {
    ty: OSMElementType,
    role: Option<String>,
    id: i64,
    text_patch: ast::TextPatch,
}

impl OSMPaintObj {
    pub fn new_node(id: i64) -> Self {
        OSMPaintObj {
            ty: OSMElementType::Node,
            role: None,
            id,
            text_patch: ast::TextPatch::new(),
        }
    }
    pub fn new_way(id: i64) -> Self {
        OSMPaintObj {
            ty: OSMElementType::Way,
            role: None,
            id,
            text_patch: ast::TextPatch::new(),
        }
    }
    pub fn new_relation(id: i64) -> Self {
        OSMPaintObj {
            ty: OSMElementType::Relation,
            role: None,
            id,
            text_patch: ast::TextPatch::new(),
        }
    }
}

fn wildcard_match(str: &str, match_str: &str) -> bool {
    if match_str == "*" {
        return true;
    }
    // Performance optimization: only use Regex if needed
    if !match_str
        .chars()
        .any(|c| matches!(c, '*' | '?' | '|' | '[' | ']' | '{' | '}' | '\\'))
    {
        return str == match_str;
    }
    let regex = match_str.replace("*", ".*");
    let re = regex::Regex::new(&format!("^{}$", regex)).unwrap();
    re.is_match(str)
}

fn match_predicate(
    osm_file: &osmpbf::OsmFile,
    paint_obj: &OSMPaintObj,
    filter: &style_file::ast::FilterExpr,
) -> bool {
    match filter {
        FilterExpr::Filter(Filter::IsNode) => {
            return paint_obj.ty == OSMElementType::Node;
        }
        FilterExpr::Filter(Filter::IsWay) => {
            return paint_obj.ty == OSMElementType::Way;
        }
        FilterExpr::Filter(Filter::IsRelation) => {
            return paint_obj.ty == OSMElementType::Relation;
        }
        FilterExpr::Filter(Filter::MatchRole(match_role)) => {
            if let Some(role) = &paint_obj.role {
                return wildcard_match(role, match_role);
            }
            return false;
        }
        FilterExpr::Filter(Filter::Match(key, value)) => match paint_obj.ty {
            OSMElementType::Node => {
                if let Some(node) = osm_file.get_node(paint_obj.id) {
                    if let Some(val) = node.get_tag_value(&key) {
                        return wildcard_match(val, value);
                    }
                }
                return false;
            }
            OSMElementType::Way => {
                if let Some(way) = osm_file.get_way(paint_obj.id) {
                    if let Some(val) = way.get_tag_value(&key) {
                        return wildcard_match(val, value);
                    }
                }
                return false;
            }
            OSMElementType::Relation => {
                if let Some(rel) = osm_file.get_relation(paint_obj.id) {
                    if let Some(val) = rel.get_tag_value(&key) {
                        return wildcard_match(val, value);
                    }
                }
                return false;
            }
        },
        FilterExpr::And(left, right) => {
            return match_predicate(osm_file, paint_obj, left)
                && match_predicate(osm_file, paint_obj, right);
        }
        FilterExpr::Or(left, right) => {
            return match_predicate(osm_file, paint_obj, left)
                || match_predicate(osm_file, paint_obj, right);
        }
        FilterExpr::Not(expr) => {
            return !match_predicate(osm_file, paint_obj, expr);
        }
    }
}

fn mercator_projection(lat: f64, lon: f64) -> (f64, f64) {
    let x = (lon / 1e7).to_radians();
    let sin_lat = (lat / 1e7).to_radians().sin();
    let y = ((1.0 + sin_lat) / (1.0 - sin_lat)).ln() / 2.0;
    (
        x / (2.0 * std::f64::consts::PI),
        y / (2.0 * std::f64::consts::PI),
    )
}

impl<'a> Renderer<'a> {
    pub fn new(meta: &'a ast::Meta, cr: &'a cairo::Context, osm_file: &'a osmpbf::OsmFile) -> Self {
        Renderer {
            meta,
            cr,
            osm_file,
            unit_scale: meta.dpi / 25.4,
            rng: rand::rng(),
        }
    }

    pub fn init(&mut self) {
        self.cr.set_antialias(cairo::Antialias::Good);
        self.cr.set_line_cap(cairo::LineCap::Round);
        self.cr.set_line_join(cairo::LineJoin::Round);
    }

    fn world_to_screen(&self, lat: i64, lon: i64) -> (f64, f64) {
        let (x, y) = mercator_projection(lat as f64, lon as f64);
        let (ox, oy) = mercator_projection(self.meta.center.0, self.meta.center.1);
        let mercator_space_coords = (x - ox, -(y - oy));
        const EARTH_CIRCUMFERENCE: f64 = 40_075_000_000.0; // [mm]
        let center_lat = self.meta.center.0 as f64 / 1e7;
        let pixels_per_world_mm = self.meta.dpi / 25.4 / self.meta.scale;
        let ground_dist_scale =
            EARTH_CIRCUMFERENCE * center_lat.to_radians().cos() * pixels_per_world_mm;
        let (w, h) = (
            self.meta.width_pixels() as f64,
            self.meta.height_pixels() as f64,
        );
        let x = mercator_space_coords.0 * ground_dist_scale + w / 2.0;
        let y = mercator_space_coords.1 * ground_dist_scale + h / 2.0;
        (x, y)
    }

    pub fn paint(&mut self, commands: &Vec<Command>) {
        let mut selection = vec![];
        for node in self.osm_file.nodes() {
            selection.push(OSMPaintObj::new_node(node.id()));
        }
        for way in self.osm_file.ways() {
            selection.push(OSMPaintObj::new_way(way.id()));
        }
        for rel in self.osm_file.relations() {
            selection.push(OSMPaintObj::new_relation(rel.id()));
        }
        self.paint_list(&mut selection, commands);
    }

    fn paint_list(&mut self, selection: &mut Vec<OSMPaintObj>, commands: &Vec<Command>) {
        for command in commands {
            match command {
                Command::Filter(FilterType::Keep, expr) => {
                    println!("Filter keep {:?}", expr);
                    selection.retain(|el| match_predicate(self.osm_file, el, expr));
                }
                Command::Filter(FilterType::Remove, expr) => {
                    println!("Filter remove {:?}", expr);
                    selection.retain(|el| !match_predicate(self.osm_file, el, expr));
                }
                Command::Take(n) => {
                    selection.truncate(*n);
                }
                Command::DrawFunc { ty, args } => {
                    println!("Draw {} elements", selection.len());
                    match ty.as_str() {
                        "Polyfill" => {
                            self.polyfill(&selection, args);
                        }
                        "Outline" => {
                            self.outline(&selection, args);
                        }
                        "Dot" => {
                            self.dot(&selection, args);
                        }
                        "Text" => {
                            self.text(&selection, args);
                        }
                        _ => {
                            println!("Unknown draw function: {}", ty);
                        }
                    }
                }
                Command::OffsetText { key, offsets } => {
                    for el in selection.iter_mut() {
                        let val = match el.ty {
                            OSMElementType::Node => self
                                .osm_file
                                .get_node(el.id)
                                .unwrap()
                                .get_tag_value(&key)
                                .unwrap_or(&"".to_string())
                                .clone(),
                            OSMElementType::Way => self
                                .osm_file
                                .get_way(el.id)
                                .unwrap()
                                .get_tag_value(&key)
                                .unwrap_or(&"".to_string())
                                .clone(),
                            OSMElementType::Relation => self
                                .osm_file
                                .get_relation(el.id)
                                .unwrap()
                                .get_tag_value(&key)
                                .unwrap_or(&"".to_string())
                                .clone(),
                        };
                        if let Some(patch) = offsets.get(&val) {
                            el.text_patch = patch.clone();
                        }
                    }
                }
                Command::Sub(cmds) => {
                    for el in selection.iter() {
                        if el.ty == OSMElementType::Relation {
                            let rel = self.osm_file.get_relation(el.id).unwrap();
                            let mut sub_selection = vec![];
                            for (sub_ty, sub_id) in &rel.data().members {
                                let role_str = self.osm_file.get_string(sub_id.role_sid).unwrap();
                                let ty = match sub_ty {
                                    osmpbf::OsmRelationMemberType::Node => OSMElementType::Node,
                                    osmpbf::OsmRelationMemberType::Way => OSMElementType::Way,
                                    osmpbf::OsmRelationMemberType::Relation => {
                                        OSMElementType::Relation
                                    }
                                };
                                sub_selection.push(OSMPaintObj {
                                    ty,
                                    role: Some(role_str.clone()),
                                    id: sub_id.ref_id,
                                    text_patch: ast::TextPatch {
                                        offset: None,
                                        rename: None,
                                        scale: None,
                                    },
                                });
                            }
                            self.paint_list(&mut sub_selection, &cmds);
                        }
                    }
                }
            }
        }
    }

    fn draw_way(&self, way: &osmpbf::OsmWayData, move_first: bool) {
        let mut is_first = move_first;
        for point in &way.refs {
            if let Some(node) = self.osm_file.get_node(*point) {
                let (map_x, map_y) = self.world_to_screen(node.data().lat, node.data().lon);
                if is_first {
                    self.cr.move_to(map_x, map_y);
                    is_first = false;
                } else {
                    self.cr.line_to(map_x, map_y);
                }
            }
        }
    }

    fn relation_to_multipolygon(&self, rel: &osmpbf::OsmRelationData) -> Vec<Vec<i64>> {
        let mut ways = vec![];
        for point in &rel.members {
            if point.0 != osmpbf::OsmRelationMemberType::Way {
                continue;
            }
            if let Some(way) = self.osm_file.get_way(point.1.ref_id) {
                ways.push(way);
            }
        }
        let mut res_polygons = vec![];
        while ways.len() > 0 {
            let mut curr_ring = vec![];
            let way0 = ways.remove(0);
            curr_ring.extend(way0.data().refs.iter());
            loop {
                let last_node = curr_ring[curr_ring.len() - 1];
                if curr_ring[0] == last_node {
                    break;
                }
                let mut found = false;
                for i in 0..ways.len() {
                    if ways[i].data().refs[0] == last_node {
                        curr_ring.extend(ways[i].data().refs.iter());
                        ways.remove(i);
                        found = true;
                        break;
                    }
                    if ways[i].data().refs[ways[i].data().refs.len() - 1] == last_node {
                        let mut refs = ways[i].data().refs.clone();
                        refs.reverse();
                        curr_ring.extend(refs);
                        ways.remove(i);
                        found = true;
                        break;
                    }
                }
                if !found {
                    break;
                }
            }
            res_polygons.push(curr_ring);
        }
        res_polygons
    }

    fn draw_relation_ways(
        &self,
        rel: &osmpbf::OsmFileElement<osmpbf::OsmRelationData>,
        close_path: bool,
    ) -> bool {
        let multipoly = self.relation_to_multipolygon(rel.data());
        for poly in multipoly {
            let mut is_first = true;
            for point in &poly {
                if let Some(node) = self.osm_file.get_node(*point) {
                    let (map_x, map_y) = self.world_to_screen(node.data().lat, node.data().lon);
                    if is_first {
                        self.cr.move_to(map_x, map_y);
                        is_first = false;
                    } else {
                        self.cr.line_to(map_x, map_y);
                    }
                }
            }
            if close_path {
                self.cr.close_path();
            }
        }
        true
    }

    fn polyfill(&mut self, els: &Vec<OSMPaintObj>, args: &HashMap<String, ast::FuncArg>) {
        let alpha = if let Some(ast::FuncArg::Float(alpha)) = args.get("alpha") {
            *alpha
        } else {
            1.0
        };
        if let Some(ast::FuncArg::Color(ast::Color { r, g, b })) = args.get("color") {
            self.cr.set_source_rgba(
                *r as f64 / 255.0,
                *g as f64 / 255.0,
                *b as f64 / 255.0,
                alpha,
            );
        }
        let rand_color = if let Some(ast::FuncArg::RandomColor) = args.get("color") {
            true
        } else {
            false
        };
        for el in els {
            if rand_color {
                let r = self.rng.random::<f64>() * 0.2 + 0.8;
                let g = self.rng.random::<f64>() * 0.2 + 0.8;
                let b = self.rng.random::<f64>() * 0.2 + 0.8;
                self.cr.set_source_rgba(r, g, b, alpha);
            }
            match el.ty {
                OSMElementType::Node => {}
                OSMElementType::Way => {
                    let way = self.osm_file.get_way(el.id).unwrap();
                    self.draw_way(way.data(), true);
                    self.cr.close_path();
                    self.cr.clip();
                    let _ = self.cr.paint();
                    self.cr.reset_clip();
                }
                OSMElementType::Relation => {
                    let rel = self.osm_file.get_relation(el.id).unwrap();
                    self.draw_relation_ways(&rel, true);

                    self.cr.clip();

                    let _ = self.cr.paint();
                    self.cr.reset_clip();
                }
            }
        }
    }

    fn outline(&mut self, els: &Vec<OSMPaintObj>, args: &HashMap<String, ast::FuncArg>) {
        let alpha = if let Some(ast::FuncArg::Float(alpha)) = args.get("alpha") {
            *alpha
        } else {
            1.0
        };
        if let Some(ast::FuncArg::Color(ast::Color { r, g, b })) = args.get("color") {
            self.cr.set_source_rgba(
                *r as f64 / 255.0,
                *g as f64 / 255.0,
                *b as f64 / 255.0,
                alpha,
            );
        }
        let rand_color = if let Some(ast::FuncArg::RandomColor) = args.get("color") {
            true
        } else {
            false
        };
        let mut width = self.unit_scale;
        if let Some(ast::FuncArg::Float(w)) = args.get("width") {
            width *= *w;
        }
        if let Some(ast::FuncArg::String(dash)) = args.get("dash") {
            let dashes = dash
                .split(",")
                .map(|x| x.parse::<f64>().unwrap())
                .collect::<Vec<f64>>();
            self.cr.set_dash(&dashes, 0.0);
        }
        self.cr.set_line_width(width);
        for el in els {
            if rand_color {
                let r = self.rng.random::<f64>() * 0.2 + 0.8;
                let g = self.rng.random::<f64>() * 0.2 + 0.8;
                let b = self.rng.random::<f64>() * 0.2 + 0.8;
                self.cr.set_source_rgb(r, g, b);
            }
            match el.ty {
                OSMElementType::Node => {}
                OSMElementType::Way => {
                    let way = self.osm_file.get_way(el.id).unwrap();
                    self.draw_way(way.data(), true);
                }
                OSMElementType::Relation => {
                    let rel = self.osm_file.get_relation(el.id).unwrap();
                    self.draw_relation_ways(&rel, false);
                }
            }
        }
        let _ = self.cr.stroke();
        self.cr.set_dash(&[], 0.0);
    }

    fn dot(&mut self, els: &Vec<OSMPaintObj>, args: &HashMap<String, ast::FuncArg>) {
        if let Some(ast::FuncArg::Color(ast::Color { r, g, b })) = args.get("color") {
            self.cr
                .set_source_rgb(*r as f64 / 255.0, *g as f64 / 255.0, *b as f64 / 255.0);
        }
        let rand_color = if let Some(ast::FuncArg::RandomColor) = args.get("color") {
            true
        } else {
            false
        };
        let mut radius = self.unit_scale;
        if let Some(ast::FuncArg::Float(w)) = args.get("radius") {
            radius *= *w;
        }
        for el in els {
            if rand_color {
                let r = self.rng.random::<f64>() * 0.2 + 0.8;
                let g = self.rng.random::<f64>() * 0.2 + 0.8;
                let b = self.rng.random::<f64>() * 0.2 + 0.8;
                self.cr.set_source_rgb(r, g, b);
            }
            match el.ty {
                OSMElementType::Node => {
                    let node = self.osm_file.get_node(el.id).unwrap();
                    let (map_x, map_y) = self.world_to_screen(node.data().lat, node.data().lon);
                    self.cr
                        .arc(map_x, map_y, radius, 0.0, 2.0 * std::f64::consts::PI);
                    self.cr.fill().unwrap();
                }
                OSMElementType::Way => {
                    // Center of mass
                    let way = self.osm_file.get_way(el.id).unwrap();
                    let mut x = 0;
                    let mut y = 0;
                    for point in &way.data().refs {
                        if let Some(node) = self.osm_file.get_node(*point) {
                            x += node.data().lon;
                            y += node.data().lat;
                        }
                    }
                    x /= way.data().refs.len() as i64;
                    y /= way.data().refs.len() as i64;
                    let (map_x, map_y) = self.world_to_screen(y, x);
                    self.cr
                        .arc(map_x, map_y, radius, 0.0, 2.0 * std::f64::consts::PI);
                    self.cr.fill().unwrap();
                }
                OSMElementType::Relation => {
                    let rel = self.osm_file.get_relation(el.id).unwrap();
                    for (ty, info) in &rel.data().members {
                        match ty {
                            osmpbf::OsmRelationMemberType::Node => {
                                if let Some(node) = self.osm_file.get_node(info.ref_id) {
                                    let (map_x, map_y) =
                                        self.world_to_screen(node.data().lat, node.data().lon);
                                    self.cr.arc(
                                        map_x,
                                        map_y,
                                        radius,
                                        0.0,
                                        2.0 * std::f64::consts::PI,
                                    );
                                    self.cr.fill().unwrap();
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    fn text(&mut self, els: &Vec<OSMPaintObj>, args: &HashMap<String, ast::FuncArg>) {
        for el in els {
            let field = if let Some(ast::FuncArg::String(field)) = args.get("field") {
                field.clone()
            } else {
                "name".to_string()
            };
            let mut lat = 0;
            let mut lon = 0;
            let text;
            match el.ty {
                OSMElementType::Node => {
                    let node = self.osm_file.get_node(el.id).unwrap();
                    lat = node.data().lat;
                    lon = node.data().lon;
                    text = node.get_tag_value(&field).map(|v| v.clone());
                }
                OSMElementType::Way => {
                    let way = self.osm_file.get_way(el.id).unwrap();
                    for point in &way.data().refs {
                        if let Some(node) = self.osm_file.get_node(*point) {
                            lat += node.data().lat;
                            lon += node.data().lon;
                        }
                    }
                    lat /= way.data().refs.len() as i64;
                    lon /= way.data().refs.len() as i64;
                    text = way.get_tag_value(&field).map(|v| v.clone());
                }
                OSMElementType::Relation => {
                    let rel = self.osm_file.get_relation(el.id).unwrap();
                    for (_, info) in &rel.data().members {
                        if let Some(node) = self.osm_file.get_node(info.ref_id) {
                            lat += node.data().lat;
                            lon += node.data().lon;
                        }
                    }
                    lat /= rel.data().members.len() as i64;
                    lon /= rel.data().members.len() as i64;
                    text = rel.get_tag_value(&field).map(|v| v.clone());
                }
            }
            if let Some(val) = text {
                let mut pango_font = pangocairo::pango::FontDescription::new();
                pango_font.set_weight(pangocairo::pango::Weight::Bold);
                let font_size = if let Some(ast::FuncArg::Float(size)) = args.get("size") {
                    *size * self.unit_scale
                } else {
                    12.0 * self.unit_scale
                };
                let size_delta = el.text_patch.scale.unwrap_or(0.0) * self.unit_scale;
                if let Some(ast::FuncArg::String(family)) = args.get("font_family") {
                    pango_font.set_family(family.as_str());
                };

                let lyt = pangocairo::functions::create_layout(&self.cr);
                lyt.set_alignment(pangocairo::pango::Alignment::Center);
                lyt.set_line_spacing(0.6);
                pango_font.set_size((font_size + size_delta) as i32 * pangocairo::pango::SCALE);
                lyt.set_font_description(Some(&pango_font));
                if let Some(rename) = &el.text_patch.rename {
                    lyt.set_text(rename);
                } else {
                    lyt.set_text(&val);
                }
                let (_, rect) = lyt.pixel_extents();
                let (mut map_x, mut map_y) = self.world_to_screen(lat, lon);
                if let Some(offset) = el.text_patch.offset {
                    map_x += offset.0 * self.unit_scale;
                    map_y += offset.1 * self.unit_scale;
                }
                self.cr.move_to(map_x, map_y);
                self.cr
                    .rel_move_to(-rect.width() as f64 / 2.0, -rect.height() as f64 / 2.0);
                if let Some(ast::FuncArg::Color(ast::Color { r, g, b })) = args.get("color") {
                    self.cr
                        .set_source_rgb(*r as f64 / 255.0, *g as f64 / 255.0, *b as f64 / 255.0);
                }
                pangocairo::functions::show_layout(&self.cr, &lyt);
            }
        }
    }
}
