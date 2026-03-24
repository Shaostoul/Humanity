USE NS project_universe;
USE DB mathematics;

DEFINE TABLE geometry SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE geometry TYPE string;
DEFINE FIELD formula         ON TABLE geometry TYPE string;
DEFINE FIELD variables       ON TABLE geometry TYPE array;
DEFINE FIELD description     ON TABLE geometry TYPE string;
DEFINE FIELD applications    ON TABLE geometry TYPE array;
DEFINE FIELD units           ON TABLE geometry TYPE string;

DEFINE INDEX unique_geometry ON TABLE geometry COLUMNS name UNIQUE;

CREATE geometry:sphere_surface SET
  name = "Sphere Surface Area",
  formula = "A = 4πr²",
  variables = ["A (surface area)", "r (radius)"],
  description = "Calculates the surface area of a sphere.",
  applications = ["Space Habitats", "Planetary Bodies"],
  units = "m²";

CREATE geometry:sphere_volume SET
  name = "Sphere Volume",
  formula = "V = (4/3)πr³",
  variables = ["V (volume)", "r (radius)"],
  description = "Calculates the volume of a sphere.",
  applications = ["Storage Spheres", "Fuel Tanks", "Habitat Domes"],
  units = "m³";

CREATE geometry:cylinder_volume SET
  name = "Cylinder Volume",
  formula = "V = πr²h",
  variables = ["V (volume)", "r (radius)", "h (height)"],
  description = "Calculates the volume of a cylinder.",
  applications = ["Fuel Tanks", "Oxygen Storage", "Cargo Holds"],
  units = "m³";

CREATE geometry:cylinder_surface SET
  name = "Cylinder Surface Area",
  formula = "A = 2πr² + 2πrh",
  variables = ["A (surface area)", "r (radius)", "h (height)"],
  description = "Calculates the surface area of a cylinder.",
  applications = ["Structural Design", "Habitat Modules"],
  units = "m²";

CREATE geometry:cone_volume SET
  name = "Cone Volume",
  formula = "V = (1/3)πr²h",
  variables = ["V (volume)", "r (radius)", "h (height)"],
  description = "Calculates the volume of a cone.",
  applications = ["Re-entry Capsules", "Rocket Nose Cones", "Aerodynamic Structures"],
  units = "m³";

CREATE geometry:pythagorean SET
  name = "Pythagorean Theorem",
  formula = "a² + b² = c²",
  variables = ["a (side 1)", "b (side 2)", "c (hypotenuse)"],
  description = "Relates the sides of a right triangle.",
  applications = ["Navigation", "Triangulation", "Spatial Measurement"],
  units = "m";

CREATE geometry:distance_3d SET
  name = "Distance Formula (3D)",
  formula = "d = √((x₂-x₁)² + (y₂-y₁)² + (z₂-z₁)²)",
  variables = ["d (distance)", "x₁,y₁,z₁ (point 1)", "x₂,y₂,z₂ (point 2)"],
  description = "Calculates the distance between two points in 3D space.",
  applications = ["Navigation", "Spatial Positioning", "Inter-object Distances"],
  units = "m";

CREATE geometry:rect_prism_volume SET
  name = "Rectangular Prism Volume",
  formula = "V = lwh",
  variables = ["V (volume)", "l (length)", "w (width)", "h (height)"],
  description = "Calculates the volume of a rectangular prism.",
  applications = ["Cargo Modules", "Storage Compartments"],
  units = "m³";

CREATE geometry:triangle_area SET
  name = "Triangle Area",
  formula = "A = (1/2)bh",
  variables = ["A (area)", "b (base)", "h (height)"],
  description = "Calculates the area of a triangle.",
  applications = ["Structural Supports", "Solar Sails", "Spacecraft Panels"],
  units = "m²";
