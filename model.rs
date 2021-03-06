use io::{Reader, ReaderUtil};
use std::sort;
use math3d::*;

pub type polysoup = { vertices: ~[vec3], indices: ~[uint], normals: ~[vec3] };

pub type mesh = { polys: polysoup, kd_tree: kd_tree, bounding_box: aabb };

pub enum axis {
    pub x,
    pub y,
    pub z
}

pub type kd_tree = { nodes : ~[kd_tree_node], root: uint };

pub enum kd_tree_node {
    pub leaf( u32, u32 ),
    pub node( axis, f32, u32 )
}

fn find_split_plane( distances: &[f32], indices: &[uint], faces: &[uint] ) -> f32 {

    let mut face_distances = ~[];
    for faces.each |f| {
        face_distances.push(distances[indices[*f*3u]]);
        face_distances.push(distances[indices[*f*3u+1u]]);
        face_distances.push(distances[indices[*f*3u+2u]]);
    }

    let mut sorted_distances = sort::merge_sort( |a,b| *a<*b, face_distances );
    let n = vec::len(sorted_distances);
    if n % 2u == 0u {
        sorted_distances[ n/2u ]
    } else {
        (sorted_distances[ n/2u -1u] + sorted_distances[ n/2u ]) * 0.5f32
    }
}

fn split_triangles( splitter: f32, distances: &[f32], indices: &[uint], faces: &[uint] ) -> {l:~[uint],r:~[uint]} {
    let mut l = ~[];
    let mut r = ~[];

    for faces.each |f| {
        let f = *f;
        let d0 = distances[indices[f*3u   ]];
        let d1 = distances[indices[f*3u+1u]];
        let d2 = distances[indices[f*3u+2u]];

        let maxdist = f32::fmax(d0, f32::fmax(d1, d2));
        let mindist = f32::fmin(d0, f32::fmin(d1, d2));

        if mindist <= splitter {
            l += [f];
        }

        if maxdist >= splitter {
            r += [f];
        }
    }

    {l:l, r:r}
}

fn build_leaf(
    kd_tree_nodes : &mut ~[kd_tree_node],
    new_indices: &mut ~[uint],
    indices: &[uint],
    faces: &[uint]
) -> uint {

    let next_face_ix : u32 = (new_indices.len() as u32) / 3u32;
    kd_tree_nodes.push(leaf( next_face_ix, (vec::len(faces) as u32) ));

    for faces.each |f| {
        let f = *f;
        *new_indices += &[ indices[f*3u], indices[f*3u+1u], indices[f*3u+2u] ];
    }
    return kd_tree_nodes.len() - 1u;
}

fn build_kd_tree(
    kd_tree_nodes : &mut ~[kd_tree_node],
    new_indices: &mut ~[uint],
    maxdepth: uint,
    xdists: &r/[f32],
    ydists: &r/[f32],
    zdists: &r/[f32],
    aabbmin: vec3,
    aabbmax: vec3,
    indices: &[uint],
    faces: &[uint] ) -> uint {

    if maxdepth == 0u || vec::len(faces) <= 15u {
        return build_leaf( kd_tree_nodes, new_indices, indices, faces );
    }

    let extent = sub(aabbmax, aabbmin);
        let axis = if extent.x > extent.y && extent.x > extent.z {
            x
        } else {
            if extent.y > extent.z { y } else { z }
        };

    let mut dists;
    match axis {
        x() => { dists = xdists }
        y() => { dists = ydists }
        z() => { dists = zdists }
    };

    let s = find_split_plane( dists, indices, faces );
    let {l,r} = split_triangles( s, dists, indices, faces );

    // Stop when there's too much overlap between the two halves
    if (vec::len(l) + vec::len(r)) as f32 > vec::len(faces) as f32 * 1.5f32 {
        return build_leaf( kd_tree_nodes, new_indices, indices, faces );
    }

    // adjust bounding boxes for children
    let mut left_aabbmax;
    let mut right_aabbmin;
    match axis {
        x() => {
            left_aabbmax = vec3 { x: s, .. aabbmax };
            right_aabbmin = vec3 { x: s, .. aabbmin };
        }
        y() => {
            left_aabbmax = vec3 { y: s, .. aabbmax };
            right_aabbmin = vec3 { y: s, .. aabbmin };
        }
        z() => {
            left_aabbmax = vec3 { z: s, .. aabbmax };
            right_aabbmin = vec3 { z: s, .. aabbmin };
        }
    };

    // allocate node from nodes-array, and recursively build children
    let ix = kd_tree_nodes.len();
    vec::push( kd_tree_nodes, node(axis,0f32,0u32));

    build_kd_tree(
        kd_tree_nodes,
        new_indices,
        maxdepth - 1u,
        xdists,
        ydists,
        zdists,
        aabbmin,
        left_aabbmax,
        indices,
        l
    );
    // left child ix is implied to be ix+1

    let right_child_ix = build_kd_tree(
        kd_tree_nodes,
        new_indices,
        maxdepth - 1u,
        xdists,
        ydists,
        zdists,
        right_aabbmin,
        aabbmax,
        indices,
        r
    );

    kd_tree_nodes[ix] = node(axis, s as f32, right_child_ix as u32);

    return ix;
}

pub fn count_kd_tree_nodes( t: kd_tree ) -> {depth:uint, count:uint} {
    match t.nodes[t.root] {
        node(_,_,r) => {
            let {depth:d0,count:c0} = count_kd_tree_nodes( {root: t.root+1u, .. t} );
            let {depth:d1,count:c1} = count_kd_tree_nodes( {root: (r as uint), .. t} );
            return {depth: uint::max(d0, d1)+1u, count: c0+c1+1u };
        }
        leaf(_, _) => {
            return { depth:1u, count:1u };
        }
    }
}

pub fn read_mesh(fname: &str) -> mesh {
    io::print("Reading model file...");
    let polys = read_polysoup( fname );

    io::print("Building kd-tree... ");

    // just create a vector of 0..N-1 as our face array
    let max_tri_ix = vec::len(polys.indices)/3u -1u;
   let mut faces = ~[];
   let mut fii = 0u;
   while fii < max_tri_ix {
      faces.push(fii);
      fii += 1u
   }

    let mut aabbmin = vec3(f32::infinity, f32::infinity, f32::infinity);
    let mut aabbmax = vec3(f32::neg_infinity, f32::neg_infinity, f32::neg_infinity);
    for polys.vertices.each |v| {
        aabbmin = math3d::min(*v, aabbmin);
        aabbmax = math3d::max(*v, aabbmax);
    }

    let downscale = 1.0f32 / math3d::length(sub(aabbmax,aabbmin));
    let offset = scale(add(aabbmin, aabbmax), 0.5f32);

    let mut transformed_verts = ~[];


    for polys.vertices.each |v| {
        transformed_verts.push(scale(sub(*v, offset), downscale));
    }

    aabbmin = scale(sub(aabbmin, offset), downscale);
    aabbmax = scale(sub(aabbmax, offset), downscale);

    // de-mux vertices for easier access later
    let mut xdists = ~[];
    let mut ydists = ~[];
    let mut zdists = ~[];

    for transformed_verts.each |v| {
        xdists.push(v.x);
        ydists.push(v.y);
        zdists.push(v.z);
    }

    let mut nodes = ~[];
    let mut new_indices = ~[];
    let rootnode  = build_kd_tree(
                        &mut nodes,
                        &mut new_indices,
                        100u,
                        xdists,
                        ydists,
                        zdists,
                        aabbmin,
                        aabbmax,
                        polys.indices,
                        faces);

    { polys: {vertices: transformed_verts, indices: new_indices, .. polys}, kd_tree: { nodes: nodes, root: rootnode } , bounding_box: {min: aabbmin, max: aabbmax} }

}

#[inline(always)]
fn parse_faceindex(s: ~str) ->  uint {

    // check for '/', the vertex index is the first
    let ix_str = match str::find_char(s, '/'){
        option::Some(slash_ix) => {
            str::substr(s, 0u, slash_ix)
        }
        _ => {
            s
        }
    };

    option::get(&uint::from_str(ix_str))-1u
}

fn read_polysoup(fname: &str) -> polysoup {
    let reader = result::get( &io::file_reader( &Path(fname) ) );
    let mut vertices = ~[];
    let mut indices = ~[];

    let mut vert_normals : ~[vec3] = ~[];

    while !reader.eof() {
        let line : ~str = reader.read_line();
        if str::is_empty(line) {
            loop;
        }

        let mut num_texcoords = 0u;
        let tokens = str::split_char(line, ' ');

        if tokens[0] == ~"v"{
            assert vec::len(tokens) == 4u;
            let v = vec3(	option::get(&float::from_str(tokens[1])) as f32,
                            option::get(&float::from_str(tokens[2])) as f32,
                            option::get(&float::from_str(tokens[3])) as f32);
            assert v.x != f32::NaN;
            assert v.y != f32::NaN;
            assert v.z != f32::NaN;

            vertices += [v];
            vert_normals += [mut vec3(0f32,0f32,0f32)];

        } else if tokens[0] == ~"f" {
            if vec::len(tokens) == 4u || vec::len(tokens) == 5u {
                let mut face_triangles = ~[];

                if vec::len(tokens) == 4u {
                    let (i0,i1,i2) = (	parse_faceindex(copy tokens[1]),
                                        parse_faceindex(copy tokens[2]),
                                        parse_faceindex(copy tokens[3]) );

                    face_triangles.push((i0, i1, i2));
                } else {
                    assert vec::len(tokens) == 5u;
                    // quad, triangulate
                    let (i0,i1,i2,i3) = (	parse_faceindex(copy tokens[1]),
                                            parse_faceindex(copy tokens[2]),
                                            parse_faceindex(copy tokens[3]),
                                            parse_faceindex(copy tokens[4]) );

                    face_triangles.push((i0,i1,i2));
                    face_triangles.push((i0,i2,i3));
                }

                for face_triangles.each |t| {
                    let (i0,i1,i2) = *t;

                    indices += [i0,i1,i2];

                    let e1 = math3d::sub(vertices[i1], vertices[i0]);
                    let e2 = math3d::sub(vertices[i2], vertices[i0]);
                    let n = normalized(cross(e1,e2));

                    vert_normals[i0] = math3d::add( vert_normals[i0], n );
                    vert_normals[i1] = math3d::add( vert_normals[i1], n );
                    vert_normals[i2] = math3d::add( vert_normals[i2], n );
                }
            } else {
                io::println(#fmt("Polygon with %u vertices found. Ignored. Currently rustray only supports 4 vertices", vec::len(tokens) - 1u));
            }
        } else if tokens[0] == ~"vt" {
            num_texcoords += 1u;
        } else if tokens[0] != ~"#" {
            io::println(#fmt("Unrecognized line in .obj file: %s", line));
        }

        if num_texcoords > 0u {
            io::println(#fmt("%u texture coordinates ignored", num_texcoords));
        }
    }

    //for v in vert_normals {
    //	v = normalized(v);
    //}

    return {	vertices: vertices,
            indices: indices,
            normals: vec::map( vert_normals, |v| normalized(*v) ) };
}
