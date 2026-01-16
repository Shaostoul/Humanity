# Construction System Design Document

## Overview

The Construction System enables players to design and build habitable structures within their allocated homestead space aboard interstellar motherships. The system combines realistic construction principles with user-friendly gameplay mechanics to create structures that can generate viable real-life blueprints.

## Core Concept

Players manage homesteads in fixed room spaces aboard humanity's first fleet of interstellar spaceships. The initial homestead size is **89 meters × 55 meters × 6 meters**, providing approximately **29,570 square meters** of floor space and **178,200 cubic meters** of volume for construction.

### Educational Objectives
- Teach fundamental construction principles
- Demonstrate material science and structural engineering
- Illustrate sustainable building practices
- Provide practical skills applicable to real-world homesteading

## Construction Approaches

### 1. Modular Section System (Recommended Primary Approach)

#### Overview
Players construct using pre-engineered modular sections that contain all necessary structural elements and calculations. This approach prioritizes ease of use while maintaining educational value.

#### Section Types

##### Wall Sections
- **Dimensions**: 3m × 3m × 0.1m (standard), 3m × 3m × 0.2m (reinforced), 3m × 6m × 0.1m (tall)
- **Materials**: Wood-framed drywall, steel-framed panels, concrete blocks, composite materials
- **Structural Elements**: Pre-calculated framing, load-bearing calculations, insulation layers
- **Connection Points**: Standardized attachment interfaces for seamless joining

##### Floor/Ceiling Sections
- **Dimensions**: 3m × 3m × 0.15m (standard), 3m × 3m × 0.3m (reinforced)
- **Materials**: Wood joists, steel beams, concrete slabs, composite panels
- **Features**: Built-in utilities channels, insulation, soundproofing

##### Foundation/Base Sections
- **Dimensions**: 3m × 3m × 0.5m (standard), 3m × 3m × 1.0m (deep foundation)
- **Materials**: Concrete, engineered stone, composite foundations
- **Features**: Moisture barriers, termite protection, utility conduits

#### Gameplay Mechanics
```rust
// Example modular placement system
struct ConstructionSection {
    dimensions: Vec3,
    material: MaterialType,
    structural_rating: StructuralRating,
    connection_points: Vec<ConnectionPoint>,
    utilities: Vec<UtilityType>,
}
```

**Placement Process:**
1. Select section type and material from construction menu
2. Preview placement with real-time structural analysis
3. Place section with automatic alignment and connection detection
4. System validates structural integrity and utility connections
5. Cost calculation and resource consumption

**Advantages:**
- Extremely user-friendly - drag-and-drop construction
- Guaranteed structural integrity
- Pre-calculated material requirements
- Educational through material property explanations
- Easy blueprint generation

### 2. Point-to-Point Construction System (Advanced Mode)

#### Overview
Advanced players can design custom structures using point-to-point placement with automatic structural calculation. This mode unlocks progressive complexity for experienced builders.

#### Core Mechanics

##### Foundation Points
- Place anchor points for structural foundations
- Auto-calculate load distribution and foundation requirements
- Material selection affects foundation depth and reinforcement

##### Wall Definition
```rust
// Point-to-point wall construction
struct WallSegment {
    start_point: Vec3,
    end_point: Vec3,
    height: f32,
    thickness: f32,
    material: MaterialType,
}
```

**Construction Process:**
1. Place start and end points for wall segments
2. System automatically calculates:
   - Stud placement (16" or 24" on-center based on load requirements)
   - Header sizing for openings
   - Shear wall requirements
   - Insulation placement
3. Real-time structural analysis shows stress points and weak areas
4. Automatic reinforcement suggestions

##### Support Structure Auto-Calculation
- **Load Analysis**: Calculates dead loads, live loads, wind loads, seismic loads
- **Beam Sizing**: Automatically determines beam dimensions based on span and load
- **Column Placement**: Suggests optimal column locations for multi-story structures
- **Bracing Systems**: Generates diagonal bracing for stability

#### Educational Integration
- Real-time display of structural calculations
- Material stress visualization
- Load path demonstrations
- Engineering principle explanations

### 3. Template-Based Construction System

#### Overview
Beginner-friendly system using pre-designed room/building templates that can be customized and expanded.

#### Template Categories

##### Basic Structures
- **Single Room**: 6m × 6m starter home
- **Two-Room**: 9m × 6m with divider wall
- **Studio Apartment**: 12m × 8m open concept

##### Advanced Templates
- **Multi-Room Home**: 15m × 10m with bedrooms, bathroom, living area
- **Workshop**: 12m × 12m with storage and workbenches
- **Greenhouse**: 8m × 6m with specialized environmental controls

#### Customization Mechanics
- Add/remove wall sections
- Modify room dimensions
- Upgrade materials
- Add specialized features (solar panels, water collection, etc.)

## Blueprint Generation System

### Real-Life Blueprint Output

#### Export Formats
- **PDF Blueprints**: Dimensioned construction drawings
- **DWG Files**: CAD-compatible files for professional use
- **Material Lists**: Comprehensive bill of materials with quantities
- **Cost Estimates**: Real-world pricing based on current market rates

#### Blueprint Contents
```rust
struct BlueprintExport {
    floor_plans: Vec<FloorPlan>,
    elevations: Vec<Elevation>,
    sections: Vec<Section>,
    material_list: MaterialList,
    structural_calculations: StructuralAnalysis,
    utility_layouts: Vec<UtilityPlan>,
}
```

##### Floor Plans
- Dimensioned layouts with wall thicknesses
- Door and window placements
- Furniture and fixture locations
- Utility routing diagrams

##### Structural Drawings
- Foundation plans with reinforcement details
- Framing layouts with member sizes
- Load calculations and structural notes
- Connection detail drawings

##### Material Specifications
- Lumber sizes and grades
- Concrete mix designs
- Hardware and fastener lists
- Insulation and vapor barrier requirements

### Technical Implementation

#### Structural Analysis Engine
```rust
struct StructuralAnalysis {
    load_calculations: LoadAnalysis,
    stress_analysis: StressAnalysis,
    material_properties: MaterialDatabase,
    safety_factors: SafetyFactors,
}

impl StructuralAnalysis {
    fn analyze_structure(&self, structure: &Structure) -> AnalysisResult {
        // Calculate loads, stresses, deflections
        // Validate against building codes
        // Generate reinforcement recommendations
    }
}
```

#### Real-World Code Compliance
- Integration with international building codes (IBC, IRC)
- Seismic zone considerations
- Wind load calculations
- Snow load analysis (for Earth-based construction)

## Material System Integration

### Material Properties Database
```rust
struct MaterialProperties {
    compressive_strength: f32,
    tensile_strength: f32,
    density: f32,
    thermal_conductivity: f32,
    cost_per_unit: f32,
    environmental_impact: f32,
    crafting_required: bool,        // Must be crafted before use
    crafting_recipe: Option<String>, // Associated crafting recipe
    quality_multipliers: QualityMultipliers, // Performance scaling
}

struct MaterialCost {
    material_type: String,
    quantity: u32,
    crafted: bool,          // Whether this material must be crafted first
    crafting_recipe: Option<String>, // Recipe ID if crafting required
    alternative_sources: Vec<String>, // Alternative material options
}

struct QualityMultipliers {
    strength_multiplier: f32,     // How crafting skill affects strength
    durability_multiplier: f32,   // How crafting skill affects longevity
    cost_multiplier: f32,         // How crafting skill affects material cost
}
```

### Material Categories
- **Traditional**: Wood, concrete, steel, brick
- **Advanced**: Carbon fiber composites, aerogel insulation, smart materials
- **Sustainable**: Recycled materials, bamboo, rammed earth, hempcrete

### Crafting Integration
**Materials must be crafted before construction use:**

#### Crafting Requirements
**Pre-construction material preparation:**
- **Raw Materials**: Basic resources (iron ore, wood logs, sand, chemicals)
- **Refined Materials**: Processed components (steel ingots, lumber, cement)
- **Finished Products**: Construction-ready materials (steel plates, drywall, concrete blocks)

#### Crafting Queue Integration
**Integration with inventory system's nested list crafting:**
```
Player Inventory (nested list)
├── Home Section
│   ├── Construction Materials
│   │   ├── Raw Resources (ore, logs, sand)
│   │   ├── Refined Materials (ingots, lumber)
│   │   └── Finished Products (plates, blocks, sheets)
│   └── Crafting Queue
│       ├── Steel Plate Recipe (active)
│       ├── Concrete Block Recipe (queued)
│       └── Lumber Processing (completed)
```

#### Construction Material Flow
```
Raw Resources → Crafting System → Finished Materials → Construction Placement
     ↓              ↓              ↓              ↓
  Mining/Gathering → Processing → Assembly → Building Integration
```

#### Crafting Dependencies
**Construction blocked by material availability:**
- **Real-time Checks**: Construction tools verify crafted material availability
- **Queue Integration**: Construction can queue crafting jobs for missing materials
- **Skill Requirements**: Higher-quality materials require advanced crafting skills
- **Time Management**: Crafting time affects construction scheduling

#### Material Quality System
**Crafting skill affects construction performance:**
- **Novice Crafting**: Basic materials with standard properties
- **Skilled Crafting**: Enhanced materials with better strength/durability
- **Master Crafting**: Premium materials with optimal performance
- **Quality Inheritance**: Construction quality reflects material quality

### Resource Management
- Material scarcity affects availability and cost
- Recycling systems for construction waste
- Supply chain simulation for remote construction
- Crafting efficiency improvements through research

## Gameplay Integration

### Homestead Development
- **Initial Space**: 89m × 55m × 6m (29,570 m² floor space)
- **Expansion Options**: Purchase additional space or height upgrades
- **Multi-Level Construction**: Unlock vertical expansion capabilities

### Economic Integration
- **Construction Costs**: Resource-based pricing system
- **Blueprint Sales**: Monetize designs for other players
- **Material Trading**: Inter-player construction material exchange

### Educational Progression
- **Beginner Tutorials**: Modular construction basics
- **Intermediate Skills**: Template customization and material science
- **Advanced Training**: Point-to-point construction and structural engineering

## User Experience Considerations

### Interface Design
- **3D Construction View**: First-person placement and editing
- **Blueprint Mode**: 2D overhead planning view
- **Material Browser**: Visual material selection with properties
- **Structural Analysis Display**: Real-time feedback on structural integrity

### Ghost Visualization System
**Three-color feedback system for construction states:**

#### Ghost Colors
- **Green Ghost**: Currently placing/preview mode - shows valid placement location
- **Blue Ghost**: Placed but not constructed - queued for building
- **Red Ghost**: Invalid placement location - cannot be placed here

#### Color Configuration
```rust
// In construction.ron
interface: (
    ghost_opacity: 0.5,
    placing_ghost_color: (0.0, 1.0, 0.0),    // Green: Currently placing
    queued_ghost_color: (0.0, 0.5, 1.0),     // Blue: Placed but queued
    invalid_ghost_color: (1.0, 0.0, 0.0),    // Red: Cannot place
    highlight_color: (0.0, 1.0, 1.0),        // Cyan: General highlighting
    valid_color: (0.0, 1.0, 0.0),            // Green: Valid operations
    error_color: (1.0, 0.0, 0.0),            // Red: Error states
)
```

### Accessibility Features
- **Simplified Mode**: Pure drag-and-drop for accessibility
- **Voice Commands**: Hands-free construction for users with mobility challenges
- **Color-Coded Feedback**: Visual indicators for structural issues

### Performance Optimization
- **LOD System**: Level of detail for distant structures
- **Instancing**: Efficient rendering of repeated modular elements
- **Async Calculations**: Background structural analysis to maintain smooth gameplay

## Technical Challenges & Solutions

### Structural Calculation Complexity
**Challenge**: Real-time structural analysis for complex buildings
**Solution**: Pre-calculated modular sections with runtime validation

### Blueprint Accuracy
**Challenge**: Ensuring blueprints meet real-world construction standards
**Solution**: Integration with engineering libraries and building code databases

### Performance vs. Realism
**Challenge**: Balancing detailed physics with smooth gameplay
**Solution**: Hybrid system with simplified physics for gameplay, detailed analysis for blueprints

## Future Expansion

### Advanced Features
- **Smart Home Integration**: Automated systems and IoT devices
- **Environmental Adaptation**: Climate-specific construction optimization
- **Modular Extensions**: Player-created construction modules
- **Multiplayer Construction**: Collaborative building projects

### Real-World Applications
- **Emergency Housing**: Rapid deployment shelter designs
- **Sustainable Communities**: Eco-friendly construction templates
- **Educational Tools**: Construction simulation for schools and training

## Homestead Room Architecture

### Alternative Architectural Approaches

#### Option 1: Flat Rectangular Containment (Current Recommendation)
The player's homestead room is a **89m × 55m × 6m steel containment vessel** serving as the foundation for all construction activities.

#### Option 2: O'Neill Cylinder Interior (Sci-Fi Realistic)
**Inspired by**: The Expanse's Nauvoo, O'Neill cylinders
- **Dimensions**: 1.2km length × 200m diameter (full cylinder)
- **Player Area**: 300m × 300m curved surface section (6° arc)
- **Artificial Gravity**: 0.3g via rotation (2 RPM)
- **Construction Surface**: Curved landscape with varied terrain
- **Atmospheric Systems**: Weather simulation, day/night cycles
- **Advantages**: Realistic sci-fi habitat, educational about space colonization
- **Challenges**: Curved surface construction math, complex gravity simulation

#### Option 3: Macross-Style Dome Habitat (City in a Bottle)
**Inspired by**: Macross generational colony ships
- **Dimensions**: 500m diameter × 100m height dome
- **Player Area**: 400m × 400m flat cityscape
- **Environment**: Earth-like atmosphere with weather, forests, rivers
- **Scale**: City-sized with neighborhoods, parks, infrastructure
- **Advantages**: Familiar city-building gameplay, rich environmental storytelling
- **Challenges**: Massive scope, complex ecosystem simulation

#### Option 4: Planetary Surface Colony (Destructible Terrain)
**Inspired by**: Space Engineers voxel terrain, No Man's Sky terrain manipulation
- **Terrain System**: Marching cubes with destructible voxels (32³ chunks)
- **Construction**: Build on natural terrain with mining/excavation capabilities
- **Material Properties**: Realistic rock/soil types with different mining difficulties
- **Environmental Hazards**: Weather erosion, seismic activity, radiation zones
- **Dynamic Events**: Meteor impacts, landslides, atmospheric breaches
- **Advantages**: True colonization feel, tactical terrain modification, emergent gameplay
- **Challenges**: Complex physics simulation, massive terrain data management

##### Voxel Terrain Integration
```rust
struct VoxelTerrain {
    chunks: HashMap<ChunkCoord, VoxelChunk>,
    material_palette: Vec<VoxelMaterial>,
    physics_world: PhysicsWorld,
    destruction_events: Vec<DestructionEvent>,
}

struct VoxelChunk {
    voxels: [[[Voxel; 32]; 32]; 32],  // 32³ voxel array
    mesh: GeneratedMesh,               // Marching cubes mesh
    physics_body: RigidBody,           // Physics collision
    modified: bool,                    // Change tracking
}
```

**Terrain Modification Mechanics:**
- **Mining**: Progressive voxel removal with realistic material yields
- **Construction**: Place structural elements that integrate with terrain
- **Terraforming**: Large-scale terrain modification for settlement development
- **Stability**: Real-time structural analysis of terrain modifications

### Architectural Approach Comparison

| Approach | Technical Complexity | Educational Value | Gameplay Variety | Performance | Blueprint Realism |
|----------|---------------------|-------------------|------------------|-------------|-------------------|
| **Rectangular Containment** | Low | Medium | Medium | Excellent | High |
| **O'Neill Cylinder** | Very High | Very High | High | Good | Medium |
| **Macross Dome** | High | High | Very High | Fair | Low |
| **Planetary Voxels** | Very High | High | Very High | Poor-Fair | High |

#### Recommended Starting Approach: Rectangular Containment
**Why**: Balances technical feasibility with rich construction gameplay
- **Technical**: Established patterns, manageable scope
- **Educational**: Clear construction principles applicable to real world
- **Gameplay**: Focused homestead building with clear objectives
- **Performance**: Optimized for large spaces
- **Expansion**: Can add cylinder/dome/voxel features later as DLC

#### Multi-Scale Construction System Benefits
**Addressing your Space Engineers comparison:**
- **Performance**: Large grid (3m) for bulk construction, small grid (0.25m) for details only when needed
- **Thin Walls**: Ultra-thin wall system with normal mapping to hide visual thinness while maintaining structural accuracy
- **Flexibility**: Players choose appropriate scale for their construction needs
- **Blueprint Accuracy**: All scales generate dimensionally correct real-world blueprints

#### Freeform Placement System: Beyond Grid Constraints
**Sub-centimeter precision with intelligent assistance**

##### Placement Precision Levels
```rust
enum PlacementPrecision {
    GridSnapped,     // Snaps to 3m, 1m, 0.25m grids
    SurfaceAligned,  // Aligns to existing object surfaces
    Freeform,        // True freeform with sub-cm precision
    Assisted,        // AI-assisted optimal placement
}
```

**Your Multi-Layer Example - Steel Floor + Pipes + Second Floor:**
```
Height 0.00m: Steel plate floor (1cm thick)
Height 0.00m - 0.10m: 10cm diameter pipes
Height 0.11m: Second steel plate floor (1cm thick)
Height 0.12m: Carpet layer (2cm thick)
Height 0.14m+: Table and chairs (freeform placement)
```

##### Intelligent Snapping System
- **Surface Detection**: Automatically detects existing surfaces for alignment
- **Height Snapping**: Context-aware height suggestions (floor level, work surface, etc.)
- **Alignment Guides**: Visual helpers for precise positioning
- **Magnetic Attachment**: Objects "stick" to appropriate surfaces
- **Angular Snapping**: 90°, 45°, 22.5°, 5° increments with visual guides
- **Axis Locking**: Lock to specific axes for precise alignment

##### Performance Optimization for Precision
- **Selective Detail**: Only render high-precision objects when close
- **LOD Scaling**: Reduce precision at distance (cm → mm → sub-mm)
- **Batching**: Group similar precision objects for efficient rendering
- **Spatial Indexing**: Fast queries for nearby high-precision objects

#### Advanced Rotation & Angular Placement
**Beyond 90-degree constraints**

##### Rotation Precision Modes
```rust
enum RotationMode {
    Cardinal,       // 90° snaps (North/South/East/West)
    Octant,         // 45° snaps (including diagonals)
    Fine,          // 5° increments with visual guides
    Freeform,      // Continuous rotation (for advanced users)
    Assisted,      // AI-suggested optimal angles
}
```

**Angular Snapping Examples:**
- **90°**: Perfect for walls, standard furniture placement
- **45°**: Diagonal bracing, angled shelves, decorative elements
- **22.5°**: Fine architectural details, compound angles
- **5°**: Precision engineering, custom fittings, artistic placement

**Visual Feedback System:**
- **Angle Indicators**: Real-time angle display during rotation
- **Snapping Ghosts**: Preview of snapped positions
- **Alignment Lasers**: Colored lines showing axis alignment
- **Stability Warnings**: Visual alerts for unstable angular placements

#### Conduit, Pipe & Wire Systems: Flexible Utility Routing
**Path-based placement for bendable infrastructure**

##### Flexible Object Categories
```rust
enum FlexibleObjectType {
    ElectricalWire,     // Single/multi-strand cables
    Conduit,           // Protective tubing for wires
    Pipe,             // Fluid transport (water, gas, etc.)
    Hose,             // Flexible fluid transport
    CableBundle,      // Multiple wires in one sheath
    Ducting,          // Air handling systems
}
```

##### Path-Based Placement System
**Wire/Pipe Routing Like Real Construction:**
1. **Start Point**: Connect to power source, breaker box, or utility junction
2. **End Point**: Connect to appliance, outlet, or fixture
3. **Path Creation**: Draw flexible path with automatic bend calculations
4. **Obstacle Navigation**: Auto-route around walls, through penetrations
5. **Length Optimization**: Minimize excess length while maintaining service loops

**Wire/Pipe Properties:**
```rust
struct FlexibleUtility {
    path: SplinePath,              // Bézier curve for routing
    material: ConductorMaterial,   // Copper, fiber optic, PVC, etc.
    diameter: f32,                // Cross-section size
    bend_radius: f32,             // Minimum bending radius
    current_load: f32,            // Electrical capacity or flow rate
    slack_factor: f32,            // Extra length for serviceability
}
```

##### Breakage & Repair Mechanics
**Realistic Maintenance Simulation:**
- **Damage Types**: Cuts, kinks, corrosion, overloading
- **Detection**: Visual indicators, performance degradation, alarms
- **Repair Options**:
  - **Splice**: Join broken sections with connectors
  - **Replace Segment**: Swap damaged portion
  - **Reroute**: Create new path around damage
  - **Upgrade**: Replace with higher-capacity version

**Example: Broken Electrical Wire**
```
Original Path: Power Source → Junction Box → Outlet (broken here)
Repair Options:
1. Splice: Add wire nut connector at break point
2. Replace: Swap 2m damaged segment with new wire
3. Reroute: Route around damaged area through alternate path
```

#### Ship-Wide vs Home-Level Utility Management
**Hierarchical infrastructure system**

##### Government-Managed Ship Systems
**"The Grid" - Abstracted Large-Scale Infrastructure:**
- **Power Generation**: Fusion reactor → Main distribution network
- **Water Processing**: Recycling system → Main plumbing trunks
- **Life Support**: Atmospheric system → Main ventilation ducts
- **Data Network**: Central computer → Main fiber backbone

**Player Interaction Level:**
- **Breaker Box Interface**: Toggle circuits, monitor power usage
- **Main Shutoff Valves**: Control water/gas flow to home
- **Vent Registers**: Adjust local airflow (not ship-wide atmosphere)
- **Data Ports**: Connect to ship network (player manages internal wiring)

##### Player-Managed Home Systems
**Detailed Construction Within Home Boundaries:**
- **Internal Wiring**: Route power from breaker box to outlets/switches
- **Plumbing**: Connect fixtures to main water lines
- **HVAC**: Design ductwork for home climate control
- **Data Cabling**: Network home devices and entertainment systems

##### Utility Connection Points
**Ship-to-Home Interface:**
```rust
struct UtilityConnection {
    connection_type: UtilityType,    // Power/Water/Gas/Data/Air
    capacity: f32,                  // Available throughput
    connection_point: Vec3,         // Physical access location
    access_level: SecurityLevel,    // Government/Player access
    monitoring: SensorData,         // Usage tracking
}
```

**Breaker Box Example:**
```
Ship Power Feed (Government Managed)
    ↓
Home Breaker Box (Player Interface)
    ├── Circuit 1: Kitchen (Player Wired)
    ├── Circuit 2: Living Room (Player Wired)
    ├── Circuit 3: Workshop (Player Wired)
    └── Main Disconnect: Emergency cutoff
```

#### Wall Passthrough Systems: Doors, Windows & Utility Penetrations
**Dynamic wall modifications and atmospheric flow control**

##### Passthrough Categories
```rust
enum PassthroughType {
    Door,               // Personnel access with seal
    Window,             // Visual access, partial seal
    UtilityPort,        // Wire/pipe penetration
    Vent,              // Air exchange port
    EmergencyHatch,    // Pressure-equalizing access
    MaintenancePanel,  // Service access
}
```

##### Door/Window Integration
**Seamless integration with wall construction:**
- **Placement**: Doors/windows replace wall sections during construction
- **Sealing**: Automatic seal generation around openings
- **Atmospheric Control**: Pressure equalization, airlocks for hazardous areas
- **Security**: Lockable access, access control systems

**Door Types:**
- **Interior Door**: Simple access between rooms
- **Exterior Door**: Pressure-sealed with emergency protocols
- **Airlock Door**: Dual-door system for depressurization
- **Automatic Door**: Sensor-activated with access control

##### Utility Penetrations
**Wall modifications for wire/pipe routing:**
- **Pre-drilled Holes**: Standard penetrations for common utilities
- **Dynamic Cutting**: Construction tools can create custom penetrations
- **Sealing Requirements**: Automatic seal application around penetrations
- **Inspection Ports**: Access panels for maintenance

**Penetration Process:**
1. **Plan Route**: Wire/pipe path intersects wall
2. **Create Opening**: Construction tool cuts precise hole
3. **Install Penetration**: Seal around utility with grommet/foam
4. **Test Integrity**: Pressure test penetration seal

#### Atmospheric Systems: Pressure, Gases & Environmental Hazards
**Multi-room atmospheric simulation with realistic gas dynamics**

##### Atmospheric Architecture
**Hierarchical containment system:**
- **Ship Level**: Overall atmospheric envelope (vacuum/exterior)
- **Home Level**: Main pressurized container (player's homestead)
- **Room Level**: Individual room atmospheres with mixing capabilities
- **Zone Level**: Localized gas clouds and hazard areas

##### Room-Level Atmosphere Containment
**Each constructed room maintains its own atmospheric composition:**
```rust
struct RoomAtmosphere {
    bounds: AABB,                    // Room spatial boundaries
    pressure: f32,                   // Pressure in kPa
    temperature: f32,                // Temperature in Celsius
    gas_composition: GasMixture,     // Gas percentages
    volume: f32,                     // Room volume in m³
    ventilation_rate: f32,           // Air changes per hour
    hazard_level: HazardRating,      // Safety classification
}
```

**Gas Mixture Tracking:**
```rust
struct GasMixture {
    oxygen: f32,        // Percentage (21% normal)
    nitrogen: f32,      // Percentage (78% normal)
    carbon_dioxide: f32, // Percentage (0.04% normal)
    water_vapor: f32,   // Percentage (variable)
    trace_gases: HashMap<GasType, f32>, // Poisons, etc.
}
```

##### Gas Dynamics Simulation
**Hybrid approach balancing realism and performance:**

###### Flow-Based Simulation (Realistic)
- **Doorways/Windows**: Gas exchange through openings
- **Pressure Differentials**: Air movement from high to low pressure
- **Ventilation Systems**: Forced air circulation
- **Diffusion**: Gradual mixing of gases over time

**Example: Opening Door Between Rooms**
```
Room A (Pressurized: 101.3 kPa, 21% O2)
    ↔ Door Opens ↔
Room B (Depressurized: 50.0 kPa, 10% O2)

Result: Pressure equalization, gas mixing over time
- Room A pressure drops gradually
- Room B pressure rises gradually
- Oxygen levels balance between rooms
```

###### Zone-Based Simulation (Performance Optimized)
- **Gas Clouds**: Localized hazard zones instead of full fluid dynamics
- **Threshold Mixing**: Rooms mix completely when doors open
- **Ventilation Zones**: Predefined airflow patterns
- **Hazard Propagation**: Simplified spread mechanics

##### Hazardous Gas Systems
**Dynamic environmental threats from plants, equipment, and damage:**

###### Plant-Based Gas Production
**Alien plants as atmospheric modifiers:**
- **Beneficial Plants**: Oxygen production, CO2 scrubbing
- **Hazardous Plants**: Toxic gas output, allergen release
- **Interactive Growth**: Gas production rates based on plant health/size

**Example: Poisonous Alien Plant in Garden**
```
Garden Room: Plant releases neurotoxin gas
Living Room: Connected via open door

Gas Propagation:
- Initial: Gas cloud forms around plant (high concentration)
- Spread: Gradual diffusion through open door
- Effect: Living room develops hazard zone near door
- Safety: Close door to contain gas to garden
```

###### Gas Movement Mechanics
**Three movement modes based on room airflow:**

1. **Static Rooms (No Ventilation)**
   - Gas clouds persist in localized areas
   - Players can walk through/around hazard zones
   - Manual ventilation required to clear

2. **Passive Ventilation**
   - Slow natural diffusion through cracks/openings
   - Gradual mixing over time
   - Hazard zones shrink slowly

3. **Active Ventilation (HVAC)**
   - Forced air circulation clears hazards quickly
   - Directional airflow can contain or direct gases
   - Filtration systems remove specific gases

##### Atmospheric Hazard Detection & Response
**Player awareness and safety systems:**

###### Detection Methods
- **Atmospheric Sensors**: Wall-mounted gas monitors
- **Personal Alarms**: Suit-integrated hazard detection
- **Visual Indicators**: Color-coded gas clouds, warning overlays
- **System Integration**: Construction tools show atmospheric data

###### Response Options
- **Containment**: Close doors to isolate hazards
- **Ventilation**: Activate HVAC systems to clear air
- **Filtration**: Use scrubbers to remove specific gases
- **Evacuation**: Emergency protocols for severe hazards
- **PPE**: Don breathing apparatus for hazard navigation

**Example Emergency Scenario:**
```
Railgun breach creates vacuum leak in workshop.
Workshop: Pressure dropping, oxygen depleting
Living Room: Connected via door, pressure equalizing

Player Response:
1. Close workshop door (containment)
2. Activate emergency ventilation
3. Don spacesuit if needed
4. Repair breach with construction tools
```

##### Performance Optimization
**Efficient atmospheric simulation:**
- **Spatial Partitioning**: Only simulate active rooms
- **LOD Simulation**: Detailed physics near player, simplified distant
- **Event-Driven Updates**: Changes trigger calculations, not constant simulation
- **Caching**: Store stable atmospheric states
- **Abstraction**: Complex gas dynamics simplified for gameplay

#### Object Layering System: Solving the Voxel Limitation
**How to place containers and ceiling lights in the same volume**

##### Object Categories & Layering Rules
```rust
enum ObjectCategory {
    Structural,     // Walls, floors, ceilings, supports
    Functional,     // Furniture, appliances, fixtures
    Utility,        // Lighting, plumbing, electrical
    Decorative,     // Art, plants, non-functional items
    Interactive,    // Containers, machinery, usable objects
}

struct PlacementRules {
    category: ObjectCategory,
    collision_mask: CollisionMask,      // What it can collide with
    attachment_points: Vec<AttachmentPoint>, // Where it can connect
    spatial_requirements: SpatialBounds,     // Required clearance
}
```

##### Layering Examples
**Scenario: Container box and ceiling light in same 3m×3m×3m volume**

**Container (Interactive Layer):**
- **Placement**: Floor level, occupies 1m×1m×1m space
- **Collision**: Blocks floor movement, prevents overlapping furniture
- **Attachment**: Can connect to floor/structural elements

**Ceiling Light (Utility Layer):**
- **Placement**: Ceiling level, occupies 0.3m×0.3m×0.2m space
- **Collision**: Only conflicts with other ceiling-mounted objects
- **Attachment**: Requires ceiling surface for mounting

**Wall Sections (Structural Layer):**
- **Placement**: Perimeter surfaces, forms room boundaries
- **Collision**: Defines walkable space, blocks line-of-sight
- **Attachment**: Provides surfaces for lights, shelves, windows

##### Intelligent Conflict Resolution
**Physical vs Logical Conflicts:**
- **Physical**: Objects can't occupy same 3D space (container can't float through light)
- **Logical**: Objects can coexist if they serve different purposes
- **Attachment**: Objects can attach to structural elements (lights to ceilings)

**Example Resolution Engine:**
```rust
fn can_place_object(new_object: &Object, existing_objects: &[Object]) -> PlacementResult {
    for existing in existing_objects {
        // Check physical collision
        if physical_bounds_overlap(new_object, existing) {
            return PlacementResult::Blocked;
        }

        // Check logical conflicts
        if logical_conflict(new_object.category, existing.category) {
            return PlacementResult::Conflicted;
        }

        // Check attachment requirements
        if needs_attachment(new_object) && !has_valid_attachment(new_object, existing) {
            continue; // Keep checking other objects
        }
    }
    PlacementResult::Allowed
}
```

##### Practical Implementation
**Your Container + Light Example:**
1. **Volume**: 3m×3m×3m space contains:
   - Container (Interactive): 1m×1m×1m on floor
   - Ceiling Light (Utility): 0.3m×0.3m×0.2m on ceiling
   - Wall Sections (Structural): Forming room boundaries

2. **No Conflicts**: Different layers, different heights, different purposes
3. **Blueprint Generation**: All objects accurately represented in final plans

**Benefits Over Voxel Systems:**
- **Flexibility**: 10-100x fewer unique blocks needed
- **Creativity**: Players can combine objects freely
- **Realism**: Matches how real construction works
- **Performance**: Object instancing instead of unique voxel meshes

### Base Structure Design

#### Primary Construction Approach: Multi-Scale Spatial Partitioning System
**Object-based placement within flexible spatial volumes - no voxel limitations**

##### Spatial Volume Concept
**Breaking free from voxel limitations:**
- **Volume Definition**: 3m × 3m × 3m spatial containers define construction zones for organization
- **Object Freedom**: Multiple distinct objects can coexist within the same volume
- **Construction Flexibility**: Physical structures can be any size, independent of volume boundaries
- **Conflict Resolution**: Intelligent collision detection prevents physical interference
- **Layered Placement**: Structural, functional, and decorative elements can overlap safely
- **Cross-Volume Construction**: Rooms and corridors can span multiple volumes seamlessly

##### Construction Scale Independence
**Volumes organize space, not constrain construction:**
- **Small Structures**: 1m wide corridors can be built within single volumes
- **Large Structures**: Multi-story buildings span multiple volumes
- **Precise Details**: Sub-centimeter construction possible within any volume
- **Adaptive Scaling**: Construction tools automatically adjust to appropriate scale

##### Practical Examples
**Small Corridor in Large Volume:**
```
3m × 3m × 3m Spatial Volume
┌─────────────────┐
│                 │ ← Empty space (no physical constraints)
│  ┌─────────┐    │
│  │1m wide  │    │ ← 1m corridor built within volume
│  │corridor │    │
│  └─────────┘    │
│                 │
└─────────────────┘
```
**Multi-Volume Room Spanning:**
```
Volume A        Volume B        Volume C
┌─────┬─────┐   ┌─────┬─────┐   ┌─────┬─────┐
│     │     │   │     │     │   │     │     │
│  Large    │   │   Room     │   │   Spanning │
│   Room    │   │   Across    │   │  Multiple │
│           │   │  Volumes   │   │   Volumes │
└─────┴─────┘   └─────┴─────┘   └─────┴─────┘
```
**Tiny Details in Any Volume:**
```
Within any 3m³ volume, you can build:
• 1cm thick steel plates at precise heights
• 10cm diameter pipes running through walls
• 0.25m precision electronic components
• Sub-millimeter wiring and cabling
```

**Addressing Your Corridor Concern:**
**Small spaces are fully supported within the volume system:**
- **1m wide corridor**: Perfectly buildable within a single 3m³ volume
- **0.8m high crawlspace**: Can be constructed with precise height control
- **0.5m wide service tunnel**: Supported with sub-meter precision
- **Thin-walled enclosures**: 1cm thick walls at any orientation

**Volume Flexibility Examples:**
- **Narrow hallway**: Build 2m long × 1m wide × 3m high corridor in one volume
- **Low ceiling room**: 3m × 3m floor with 2.2m ceiling height
- **Confined spaces**: Utility closets, access panels, maintenance tunnels
- **Irregular shapes**: Non-rectangular spaces within volume boundaries

##### Large Scale (3m × 3m × 3m): Structural Foundation
- **Primary Use**: Major structural elements (walls, floors, ceilings, supports)
- **Volume Management**: Defines buildable space boundaries
- **Foundation Layer**: Establishes the basic spatial framework

##### Medium Scale (1m × 1m × 1m): Functional Integration
- **Object Types**: Furniture, appliances, fixtures, lighting, containers
- **Placement Rules**: Can overlap with structural elements (lights in ceilings, furniture on floors)
- **Interaction Zones**: Define usable spaces within larger volumes

##### Small Scale (0.25m × 0.25m × 0.25m): Precision Details
- **Micro Objects**: Conduits, wiring, small mechanisms, fasteners
- **High Precision**: Enables detailed engineering work
- **Integration**: Can attach to or route through larger objects

##### Micro Scale (1cm × 1cm × 1cm): Sub-Millimeter Precision
**Freeform placement with intelligent snapping**
- **Use Case**: Ultra-thin sheets, pipes, cables, precision components
- **Placement**: True freeform positioning with sub-centimeter accuracy
- **Snapping**: Intelligent surface snapping and alignment guides
- **Performance**: Selective rendering, LOD-based detail reduction
- **Integration**: Seamless attachment to all scale objects

##### Ultra-Thin Wall Construction
**Specialized system for 1cm thick walls (lava-resistant Hawaii style)**
```rust
struct ThinWall {
    thickness: f32,        // 0.01m (1cm) for ultra-thin
    material: MaterialType, // High-strength concrete/composite
    reinforcement: ReinforcementType, // Rebar mesh at 2cm intervals
    surface_treatment: SurfaceTreatment, // Lava/corrosion resistant
}
```
- **Structural Integrity**: Despite thinness, maintains strength through material properties
- **Visual Representation**: Normal-mapped surfaces hide actual thinness
- **Blueprint Accuracy**: Generates true 1cm wall specifications

#### Technical Implementation
```rust
struct HomesteadRoom {
    spatial_volumes: SpatialHashMap<VolumeId, ConstructionVolume>,
    placed_objects: Vec<PlacedObject>,
    fixed_elements: Vec<FixedElement>,  // Monorails, elevators, etc.
    atmospheric_systems: AtmosphericSystem,
    layering_engine: ObjectLayeringEngine,
    room_atmospheres: HashMap<RoomId, RoomAtmosphere>, // Per-room air systems
    gas_zones: Vec<GasZone>,        // Localized gas clouds
    passthroughs: Vec<Passthrough>, // Doors, windows, penetrations
}

struct ConstructionVolume {
    bounds: AABB,                    // 3m × 3m × 3m spatial container (organizational only)
    contained_objects: Vec<ObjectId>, // Multiple objects per volume (any size)
    structural_integrity: f32,       // Volume-level stability
    utility_connections: Vec<UtilityConnection>,
    // Note: Objects within volume can extend beyond bounds if they span volumes
}

struct PlacedObject {
    id: ObjectId,
    position: Vec3,
    rotation: Quat,
    scale: Vec3,
    category: ObjectCategory,
    collision_bounds: Vec<CollisionShape>,
    attachment_points: Vec<AttachmentPoint>,
    properties: ObjectProperties,
    placement_precision: PlacementPrecision,  // Grid/Freeform/Assisted
    snap_targets: Vec<Surface>,             // Available attachment surfaces
    rotation_mode: RotationMode,            // Angular placement precision
    flexible_path: Option<SplinePath>,      // For wires/pipes/conduits
}

struct ObjectLayeringEngine {
    category_rules: HashMap<ObjectCategory, PlacementRules>,
    conflict_resolution: ConflictResolver,
    attachment_system: AttachmentSystem,
    precision_engine: PrecisionEngine,      // Handles sub-cm placement
    snapping_system: SnappingSystem,        // Surface detection & alignment
    rotation_engine: RotationEngine,        // Angular placement & snapping
    utility_router: UtilityRouter,          // Path-based wire/pipe routing
}
```

### Fixed Infrastructure Elements

#### Resource Transport Systems
**Monorail Cargo System**: Automated cargo container transport
- **Location**: Northern wall (cargo intake), Southern wall (cargo output)
- **Capacity**: 2m × 2m × 4m cargo containers
- **Speed**: 5 m/s operational, 20 m/s express mode
- **Integration**: Seamlessly connects to player's construction (walls can be built around but not through rails)

**Elevator System**: Multi-level transport and crew access
- **Locations**: Four corners of the room
- **Capacity**: 4m × 4m × 6m (full height access)
- **Features**: Emergency stop systems, atmospheric seals, crew capacity indicators

#### Atmospheric & Life Support Integration
- **Ventilation Grates**: Fixed 2m × 2m grates in ceiling grid
- **Life Support Panels**: Wall-mounted systems for air recycling and temperature control
- **Emergency Systems**: Pressure doors, oxygen reserves, radiation shielding

#### Utility Connection Points
- **Power Grid**: High-voltage connection points every 9m along walls
- **Water Mains**: 300mm diameter pipes with access panels
- **Data Network**: Fiber optic and wireless access points
- **Waste Management**: Collection points integrated into floor grid

### Performance Optimization
- **Level of Detail (LOD)**: Base grid uses low-poly meshes at distance
- **Instancing**: Identical grid sections share geometry
- **Occlusion Culling**: Hide unseen interior sections
- **Asynchronous Loading**: Load room sections based on player proximity

### Dynamic Events & Hull Breach System

#### Railgun Impact Mechanics
**Real-time destruction and repair gameplay**
- **Impact Physics**: Railgun projectiles create penetrations with realistic ballistics
- **Decompression Events**: Hull breaches trigger atmospheric loss simulation
- **Repair Mechanics**: Players must locate and patch breaches using construction tools

##### Breach Types
- **Small Breach**: 5-15cm diameter, slow air loss, auto-seal possible
- **Medium Breach**: 30-60cm diameter, moderate decompression, requires patching
- **Large Breach**: 1-2m diameter, catastrophic loss, emergency response required

##### Repair Process
```rust
struct HullBreach {
    location: Vec3,
    size: f32,
    damage_type: DamageType,
    atmospheric_loss: f32,
    repair_materials: Vec<Material>,
    time_limit: Duration,
}
```

**Emergency Repair Protocol:**
1. **Detection**: Audio/visual alarms indicate breach location
2. **Assessment**: Scan shows breach size and structural damage
3. **Containment**: Temporary seals using emergency foam/patches
4. **Permanent Repair**: Construction system creates proper hull reinforcement
5. **Pressure Test**: Atmospheric integrity verification

#### Environmental Hazard Integration
- **Radiation Leaks**: Require shielded repair suits
- **Fire Suppression**: Automatic systems + player intervention
- **Structural Failure**: Progressive damage from sustained impacts
- **Cascade Effects**: One breach can cause secondary failures

### Construction Constraints
- **Build Height**: Maximum 5.5m (0.5m clearance from ceiling)
- **Foundation**: Must build on steel floor grid (no floating structures)
- **Clearances**: 2m clearance around fixed infrastructure elements
- **Load Limits**: Structural validation prevents overloading base supports
- **Hull Integrity**: Construction cannot compromise containment vessel

## Implementation Roadmap

### Phase 1A: Core Infrastructure (Foundation Layer)
**Duration**: 2-3 weeks | **Priority**: Critical

#### Technical Requirements
- Implement spatial volume partitioning system (3m³ containers)
- Create base steel containment vessel geometry
- Basic lighting and atmospheric effects
- Fixed infrastructure placement (monorails, elevators)
- Object layering engine foundation

#### Deliverables
- [ ] Homestead room base geometry (89m × 55m × 6m)
- [ ] Spatial volume partitioning system (3m³ containers)
- [ ] Fixed infrastructure models (monorails, elevators, vents)
- [ ] Object layering engine foundation
- [ ] Performance optimization for large-scale rendering

#### Success Criteria
- Room renders smoothly at 60fps with player movement
- Fixed infrastructure elements are properly positioned
- Spatial volume system supports multi-object placement

### Phase 1B: Modular Construction Core
**Duration**: 3-4 weeks | **Priority**: Critical

#### Technical Requirements
- Object categorization system (Structural, Functional, Utility, etc.)
- Layered placement engine with conflict resolution
- Multi-scale object models (3m, 1m, 0.25m scales)
- Material system with 3-5 basic materials
- Intelligent collision detection and attachment system
- Resource cost system integration

#### Core Features
- [ ] Object layering engine implementation
- [ ] Basic object categories and placement rules
- [ ] Wall/floor/ceiling objects with attachment points
- [ ] Container and lighting objects for testing
- [ ] Material property system (strength, cost, weight)
- [ ] Conflict resolution for multi-object volumes

#### Success Criteria
- Players can place multiple objects in same volume (container + light test case)
- Object layering prevents inappropriate conflicts
- Materials affect structural integrity
- Construction costs resources appropriately
- No performance degradation during building

### Phase 1C: Enhanced Construction Features
**Duration**: 2-3 weeks | **Priority**: High

#### Technical Requirements
- Advanced material selection (8-10 materials)
- Door and window integration with atmospheric sealing
- Wall passthrough systems (utility penetrations, doors, windows)
- Basic atmospheric simulation (pressure, gas mixing)
- Basic utility routing (power, water, data)
- Advanced rotation system (5° increments, angular snapping)
- Path-based conduit/pipe/wire placement
- Structural analysis feedback
- Freeform placement system with sub-cm precision
- Intelligent snapping and surface alignment
- Template system foundations
- Construction tool UI polish

#### Advanced Features
- [ ] Expanded material library with properties
- [ ] Door/window placement and functionality with atmospheric sealing
- [ ] Wall passthrough systems (utility penetrations)
- [ ] Basic room-level atmospheric simulation
- [ ] Gas hazard detection and visualization
- [ ] Basic utility connection visualization
- [ ] Advanced rotation system (5° increments, angular snapping)
- [ ] Path-based conduit/pipe/wire routing
- [ ] Real-time structural feedback
- [ ] Freeform placement with sub-cm precision
- [ ] Intelligent surface snapping system
- [ ] Construction undo/redo system
- [ ] Save/load construction states

#### Success Criteria
- Functional doors and windows in constructions with proper atmospheric sealing
- Wall passthroughs allow utility routing through walls
- Basic atmospheric pressure simulation works between connected rooms
- Gas hazards are visually represented and affect player safely
- Functional doors and windows in constructions
- Visual feedback for structural issues
- Freeform placement allows precise multi-layer construction (steel + pipes + carpet example)
- Advanced rotation system supports 5° increments with reliable snapping
- Path-based wire/pipe routing works for complex layouts
- Surface snapping works reliably for attachment
- Construction sessions can be saved and resumed
- Small spaces (1m corridors, 0.8m crawlspaces) can be built within large volumes

### Phase 2A: Template and Customization System
**Duration**: 3-4 weeks | **Priority**: High

#### Technical Requirements
- Pre-built room templates (single room, multi-room, workshop)
- Template customization mechanics
- Advanced material upgrades
- Environmental consideration integration
- Blueprint preview system

#### Template System
- [ ] Basic room templates (6m×6m to 15m×10m)
- [ ] Specialized templates (workshop, greenhouse)
- [ ] Template modification tools
- [ ] Material upgrade system
- [ ] Cost calculation for modifications

#### Success Criteria
- Players can start with templates and customize them
- Material upgrades affect appearance and function
- Template system reduces initial construction complexity

### Phase 2B: Advanced Construction Mechanics
**Duration**: 4-5 weeks | **Priority**: Medium

#### Technical Requirements
- Multi-level construction support
- Complex structural analysis
- Environmental adaptation (temperature, moisture)
- Advanced utility systems
- Construction automation features

#### Advanced Mechanics
- [ ] Vertical construction (multiple floors)
- [ ] Advanced structural validation
- [ ] Environmental system integration
- [ ] Complex utility networks
- [ ] Construction presets and automation

#### Success Criteria
- Multi-story buildings are structurally sound
- Environmental factors affect construction requirements
- Utility systems function across complex layouts

### Phase 3A: Blueprint Generation Foundation
**Duration**: 3-4 weeks | **Priority**: Medium

#### Technical Requirements
- 2D blueprint generation from 3D models
- Material list export
- Basic dimensioning system
- PDF export capability
- Cost estimation integration

#### Blueprint System
- [ ] Floor plan generation
- [ ] Elevation drawings
- [ ] Material quantity calculations
- [ ] Basic dimension annotations
- [ ] Export to PDF format

#### Success Criteria
- Generated blueprints are dimensionally accurate
- Material lists match construction
- PDF export works reliably

### Phase 3B: Advanced Blueprint Features
**Duration**: 3-4 weeks | **Priority**: Low

#### Technical Requirements
- Building code compliance checking
- Structural calculation export
- DWG/CAD file export
- Real-world cost estimation
- Professional blueprint formatting

#### Advanced Blueprints
- [ ] Building code validation
- [ ] Structural analysis documentation
- [ ] CAD-compatible export (DWG)
- [ ] Professional formatting and annotations
- [ ] Real-world pricing integration

#### Success Criteria
- Blueprints meet basic building code requirements
- CAD software can import generated files
- Cost estimates are market-realistic

### Phase 4: Advanced Features & Polish
**Duration**: 6-8 weeks | **Priority**: Low

#### Technical Requirements
- Multiplayer construction
- Smart home integration
- Environmental adaptation
- Advanced AI assistance
- Performance optimization

#### Future Features
- [ ] Multiplayer collaborative construction
- [ ] Automated home systems
- [ ] Climate-specific optimizations
- [ ] Construction AI assistant
- [ ] Advanced performance optimizations

#### Success Criteria
- All core features are polished and stable
- Advanced features enhance but don't break core gameplay
- Performance is optimized for complex constructions

### Phase 5: Alternative Architecture Implementation (Optional DLC)
**Duration**: 12-16 weeks | **Priority**: Optional

#### Cylinder Habitat Expansion
- [ ] Curved surface construction mathematics
- [ ] Artificial gravity simulation
- [ ] Atmospheric weather systems
- [ ] Large-scale terrain generation

#### Dome City Expansion
- [ ] Ecosystem simulation systems
- [ ] City-scale construction mechanics
- [ ] Environmental storytelling
- [ ] Multi-neighborhood management

#### Planetary Colony Expansion
- [ ] Voxel terrain system implementation
- [ ] Destructible environment mechanics
- [ ] Mining and resource extraction
- [ ] Dynamic weather and hazards

#### Hull Breach & Dynamic Events
- [ ] Railgun impact physics
- [ ] Atmospheric decompression simulation
- [ ] Emergency repair mechanics
- [ ] Progressive damage systems

#### Success Criteria
- Each alternative architecture provides unique gameplay value
- Performance remains stable across all habitat types
- Dynamic events create meaningful emergency scenarios
- All architectures support blueprint generation

## Implementation File Structure

### Recommended Folder Organization
**Dedicated `source/construction/` folder for clean separation:**

```
source/construction/
├── mod.rs                    # Main module exports and initialization
├── core.rs                   # Core ConstructionSystem and main coordinator
├── spatial.rs                # Spatial partitioning and volume management
├── objects.rs                # Object definitions, categories, and layering
├── placement.rs              # Freeform placement and collision detection
├── materials.rs              # Material properties and database
├── atmosphere.rs             # Atmospheric simulation and gas dynamics
├── utilities.rs              # Wire/pipe/conduit routing systems
├── passthroughs.rs           # Doors, windows, wall penetrations
├── ui.rs                     # Construction interface and tools
├── config.rs                 # Configuration loading and management
├── persistence.rs            # Save/load construction states
├── renderer.rs               # Rendering integration and ghost systems
└── tests.rs                  # Unit and integration tests

data/construction/
├── materials.ron            # Material definitions and properties
├── objects.ron              # Buildable object database
├── templates.ron            # Room/building templates
├── construction.ron         # System configuration (updated)
└── blueprints/              # Saved blueprint directory
```

### File Responsibilities

#### Core Files
- **`mod.rs`**: Module exports, initialization, and public API
- **`core.rs`**: Main `ConstructionSystem` struct, high-level coordination
- **`config.rs`**: Load/parse construction.ron and related config files

#### Spatial & Object Management
- **`spatial.rs`**: 3m³ volume management, spatial queries, partitioning logic
- **`objects.rs`**: `PlacedObject` definitions, object categories, layering rules
- **`placement.rs`**: Freeform placement, collision detection, snapping systems

#### Systems & Simulation
- **`materials.rs`**: Material database, properties, cost calculations
- **`atmosphere.rs`**: Room atmospheres, gas simulation, hazard systems
- **`utilities.rs`**: Path-based wire/pipe routing, connection management
- **`passthroughs.rs`**: Wall modifications, door/window systems, utility ports

#### Interface & Persistence
- **`ui.rs`**: Construction tools, interface management, user input
- **`renderer.rs`**: Ghost visualization (green/blue/red), 3D rendering integration
- **`persistence.rs`**: Save/load construction states, blueprint export/import

### Migration from Current System

#### Files to Replace/Remove
- `source/construction.rs` (current) → `source/construction/mod.rs` + `source/construction/core.rs`
- `source/construction_tool.rs` → Split into `placement.rs`, `ui.rs`, `renderer.rs`
- `source/config_construction.rs` → `source/construction/config.rs`

#### New Files to Create
- `source/construction/spatial.rs` - Spatial volume system
- `source/construction/objects.rs` - Object layering system
- `source/construction/atmosphere.rs` - Gas simulation
- `source/construction/utilities.rs` - Wire/pipe routing
- `source/construction/passthroughs.rs` - Wall modifications
- `source/construction/persistence.rs` - Save/load system

#### Data File Updates
- `data/construction.ron` - Updated with new ghost colors and config options
- `data/construction/materials.ron` - New material database
- `data/construction/objects.ron` - Buildable object definitions
- `data/construction/templates.ron` - Room/building templates

### Implementation Dependencies

#### Internal Dependencies
```
core.rs ← spatial.rs, objects.rs, placement.rs, materials.rs
placement.rs ← spatial.rs, objects.rs
atmosphere.rs ← spatial.rs
utilities.rs ← spatial.rs, placement.rs
passthroughs.rs ← spatial.rs, objects.rs
ui.rs ← placement.rs, renderer.rs
renderer.rs ← objects.rs, placement.rs
persistence.rs ← core.rs
```

#### External Dependencies
```
construction/ ← source/widgets.rs (nested list integration)
construction/ ← source/inventory.rs (material management)
construction/ ← source/renderer.rs (3D rendering)
construction/ ← source/input.rs (construction tool input)
construction/ ← source/theme.rs (UI theming)
```

### Development Phases File Creation

#### Phase 1A: Core Infrastructure
**New files to create:**
- `source/construction/mod.rs` - Basic module structure
- `source/construction/spatial.rs` - Volume partitioning foundation
- `source/construction/config.rs` - Configuration loading

#### Phase 1B: Modular Construction Core
**Additional files:**
- `source/construction/objects.rs` - Object definitions and categories
- `source/construction/placement.rs` - Basic placement system
- `source/construction/materials.rs` - Material database
- `source/construction/renderer.rs` - Ghost visualization system

#### Phase 1C: Enhanced Construction Features
**Extended files:**
- `source/construction/atmosphere.rs` - Basic atmospheric simulation
- `source/construction/passthroughs.rs` - Wall modification system
- `source/construction/utilities.rs` - Basic utility routing
- `source/construction/ui.rs` - Construction interface tools

#### Phase 2A: Template and Customization System
**Advanced features:**
- `source/construction/persistence.rs` - Save/load system
- Enhanced `objects.rs` with template system
- Enhanced `ui.rs` with customization tools

---

## Design Philosophy

The Construction System must balance **educational value** with **gameplay enjoyment** while delivering **practical utility** through viable real-life blueprints. The modular approach ensures accessibility for all players while the point-to-point system provides depth for advanced users. Every construction choice should teach real engineering principles, from basic material properties to complex structural calculations, ultimately empowering players with genuine homesteading skills applicable both in-game and in reality.
