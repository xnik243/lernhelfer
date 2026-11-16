## 8. COMPLETE VISUAL, ARCHITECTURAL & WORLD-DETAIL OVERHAUL (ANTI-GRAIN SHADER, SUB-VOXEL FURNITURE MESHER, LANDSCAPED ISLANDS & `WORLD_VERSION` MIGRATION)
Execute these four mandatory upgrades so the terrain shader is silky-smooth, furniture/windows render without floor holes, and both the Hub, Sub-Islands, and Private Islands regenerate with rich interior and exterior architectural details:

### A. Eliminate Grainy/Noisy Ground Shimmer in `Extrem + Tag` (`crates/client` WGSL Shader)
1. **Analytic Mipmap Filtering (`dpdx`/`dpdy` Noise Attenuation):**
   - At the very top of `fs_main` (outside any control flow), compute UV derivative footprint: `let fw = max(length(dpdx(world_uv)), length(dpdy(world_uv)));`.
   - Attenuate all high-frequency procedural grain/noise by `let detail_fade = clamp(1.0 - fw * 18.0, 0.0, 1.0);` so oblique and distant ground surfaces blend smoothly into clean macro-colors with zero salt-and-pepper pixel static.
2. **Soften Grass/Foliage Normals & Restrict POM to Hard Masonry:**
   - Disable Parallax Occlusion Mapping (POM) and harsh micro-normal bumps on Grass, Dirt, Sand, and Leaves (`normal_strength = 0.12 * detail_fade`, `roughness = 0.92`, `metallic = 0.0`). Restrict POM and sharp specular glints strictly to Stone Bricks, Cobblestone, Planks, and Crystals within `8.0m`.
   - Blend a smooth macro meadow gradient (`simplex_noise_2d(world_xz * 0.08)`) across grass blocks.
3. **SSAO Flat-Surface Guard:**
   - Set SSAO minimum depth-delta threshold to `0.045` so flat horizontal floors and lawns never self-darken with grainy speckles.

### B. Sub-Voxel 3D Furniture, Window & Flora Mesher (With Anti-Hole Culling Law)
Extend `crates/shared` block metadata and `crates/client` chunk mesher so decorative/architectural blocks render as detailed 3D sub-voxel models instead of `1×1×1` cubes:
1. **CRITICAL GREEDY-MESHER & COLLISION LAW (`is_opaque_full_cube`):**
   - For all sub-voxel blocks below, `is_opaque_full_cube(block_id)` MUST return `false` so adjacent solid floors, walls, and ceilings are **NEVER culled** (preventing invisible void holes underneath tables, carpets, stairs, windows, or flowers!).
   - Bypass the greedy quad-merger for sub-voxel blocks and emit their dedicated multi-box/cross geometry directly into the chunk vertex buffer.
   - **Collision Rules:** `3D-Blumen`, `Hohes Gras`, `Teppich`, and `Kronleuchter` are non-solid (`is_solid = false`, players walk cleanly through them); `Treppe` and `Stufe/Tresen` allow smooth walking step-up (`step_height = 0.55`); `Dünne Fensterscheibe`, `Geländer`, `Tisch`, and `Stuhl` have solid bounds.
2. **10 Essential Architectural & Furniture Models:**
   - `Dünne Fensterscheibe (Thin Glass Pane)`: Centered `0.12`-thick translucent glass sheet with wooden mullion frame (replaces the bulky protruding 1m glass cubes!).
   - `Treppe (Voxel Stairs)` & `Stufe / Tresen (Half-Slab / Counter)`: L-shaped 2-step stairs and `0.5`-high polished oak/stone counters for roofs, staircases, and NPC desks.
   - `Tisch (Table)` & `Stuhl / Sitzbank (Chair & Bench)`: Tabletop (`Y: 0.85..1.0`) with 4 corner legs; chairs/benches with seat (`Y: 0.45`) and backrest.
   - `Teppich (Woven Carpet)`: Flat `0.05`-thick bordered floor rug (`Rot`, `Gold`, `Blau`, `Smaragd`).
   - `Kronleuchter & Wandlaterne (Chandelier & Lantern Post)`: Wrought-iron + warm glowing crystal light (`emission = vec3(3.2, 2.1, 0.9)`).
   - `Geländer / Holzzaun (Railing & Fence)` & `Markisen-Stoff (Striped Awning Slab)`: Post-and-rail fences for cliff edges/balconies and sloped market stall canopies.
   - `Deko-Fass & Bücherstapel (Barrel & Book Stack)`: Detailed props for market stalls, storage rooms, and library tables.
   - `3D-Blumen & Hohes Gras (Cross-Mesh Flora)`: True `45°` X-crossed 3D flower meshes (*Mohnblume*, *Sonnenblume*, *Kornblume*, *Leucht-Klee*) and swaying grass tufts placed **ON TOP** of grass blocks (completely removing the flat red/yellow cubes embedded inside the lawn!).

### C. Handcrafted Building Interiors & Exterior Island Landscaping (`world_blueprints.rs` via `haiku-content`)
Overhaul the blueprints for the Hub, all Sub-Islands, and the Private Starter Island so every structure looks like a detailed, lived-in medieval/fantasy RPG build inside and out:
1. **Exterior Architecture & Island Landscaping Rules:**
   - **Buildings:** Framed dark-oak corner pillars, stone-brick foundations, recessed **Thin Glass Panes** with flower boxes underneath, overhanging pitched stair roofs, and arched entrances.
   - **Outdoor Hub & Sub-Islands:** Textured pathways (cobblestone + stone bricks + gravel border), wooden **Geländer/Zäune** along dangerous cliff edges and bridges, wooden park **Sitzbänke** under custom trees, street **Laternen-Masten**, 3D flower meadows, and market stalls furnished with **Tresen**, **Deko-Fässern**, display items, and striped **Markisen**.
2. **Richly Furnished Building Interiors:**
   - **Bibliotheks-Turm (Complete Interior Rebuild):** Remove the dark chaotic inner block clutter! Construct a bright, open 3-story gallery atrium connected by a **walkable spiral staircase (`Treppe` + `Geländer`)**, wall-flush bookshelves, cozy reading corners (**Tische**, **Stühle**, **Bücherstapel**, red/gold **Teppiche**), bright ceiling **Kronleuchter** on every floor, and the glowing **Verzauberungs-Altar** on a central dais.
   - **Bank, Markthalle & Minion-Werkstatt:** Furnish with red-and-gold **Teppich-Läufern**, polished **Schalter-Tresen (Counters)** in front of NPCs, vault bars/gold stacks in the Bank, workbenches/anvils/crates in the Workshop, seating benches, thin glass windows, and warm chandelier lighting.
   - **Private Player Islands (Detailed Starter Island & Hut):** Upgrade the private island template with a charming furnished **Starter-Hütte** (pitched stair roof, thin glass windows, interior `Tisch`, `Stuhl`, `Teppich`, `Lager-Truhe`, workbench, and warm lantern) PLUS a landscaped exterior (**Naturstein-Gartenweg** connecting Spawn, Hut, Minion Pedestal, and the Cobblestone/Shard Generator, a custom oak tree with a hanging lantern, wooden cliff fencing, and 3D wildflowers).

### D. Mandatory `WORLD_VERSION` Bump & Automatic Blueprint Migration (`crates/server`)
- **CRITICAL DB PERSISTENCE RULE:** Increment `WORLD_VERSION` in `crates/server` (e.g., to `WORLD_VERSION = 3`).
- On server startup, when `db_world_version < 3`:
  1. Automatically regenerate the public **Hub ("Campus-Marktplatz")** and all **Sub-Islands** (*Bibliotheks-Turm*, *Minion-Werkstatt*, *Prüfungs-Zitadelle*) in `campus.db` using the new furnished blueprints and 3D flora.
  2. Regenerate unedited starter chunks on **Private Player Islands** (or stamp the upgraded Starter Hut, garden path, and 3D flowers onto the starter island template while preserving any custom chests/blocks placed by players) and update `world_version = 3` in `campus.db` so every player immediately sees the new buildings and terrain on login!
- Verify via Playwright screenshots in the final gate: (1) Grain-free Hub lawn with 3D cross-mesh flowers, (2) Furnished Bank interior with counter/carpet/thin windows, (3) Bright Bibliotheks-Turm atrium with walkable stairs & chandeliers, and (4) Upgraded Private Starter Island with furnished hut and garden path.
