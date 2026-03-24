USE NS project_universe;
USE DB mathematics;

DEFINE TABLE formulas SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE formulas TYPE string;
DEFINE FIELD formula         ON TABLE formulas TYPE string;
DEFINE FIELD variables       ON TABLE formulas TYPE array;
DEFINE FIELD description     ON TABLE formulas TYPE string;
DEFINE FIELD applications    ON TABLE formulas TYPE array;
DEFINE FIELD units           ON TABLE formulas TYPE string;

DEFINE INDEX unique_formula ON TABLE formulas COLUMNS name UNIQUE;

CREATE formulas:orbital_velocity SET
  name = "Orbital Velocity",
  formula = "v = √(GM/r)",
  variables = ["v (velocity)", "G (gravitational constant)", "M (mass of central body)", "r (orbital radius)"],
  description = "Calculates the velocity required for a stable orbit around a celestial body.",
  applications = ["Calculating Stable Spacecraft Orbits"],
  units = "m/s";

CREATE formulas:escape_velocity SET
  name = "Escape Velocity",
  formula = "v = √(2GM/r)",
  variables = ["v (velocity)", "G (gravitational constant)", "M (mass of body)", "r (radius)"],
  description = "Calculates the minimum velocity needed to escape a celestial body's gravitational field.",
  applications = ["Launching from Planets or Moons"],
  units = "m/s";

CREATE formulas:kinetic_energy SET
  name = "Kinetic Energy",
  formula = "KE = ½mv²",
  variables = ["KE (kinetic energy)", "m (mass)", "v (velocity)"],
  description = "Calculates the energy of motion for a moving object.",
  applications = ["Evaluating Impacts", "Projectiles", "Spacecraft Propulsion"],
  units = "Joules";

CREATE formulas:gravitational_force SET
  name = "Gravitational Force",
  formula = "F = Gm₁m₂/r²",
  variables = ["F (force)", "G (gravitational constant)", "m₁ (mass 1)", "m₂ (mass 2)", "r (distance)"],
  description = "Calculates the gravitational force between two masses.",
  applications = ["Spacecraft Navigation", "Gravity Simulation"],
  units = "Newtons";

CREATE formulas:newtons_second SET
  name = "Newton's Second Law",
  formula = "F = ma",
  variables = ["F (force)", "m (mass)", "a (acceleration)"],
  description = "Relates force, mass, and acceleration.",
  applications = ["Engineering Propulsion Systems", "Spacecraft Maneuvers"],
  units = "Newtons";

CREATE formulas:acceleration_gravity SET
  name = "Acceleration due to Gravity",
  formula = "g = GM/r²",
  variables = ["g (acceleration)", "G (gravitational constant)", "M (mass of body)", "r (radius)"],
  description = "Calculates the acceleration due to gravity on a celestial body.",
  applications = ["Planetary Surface Exploration", "Vehicle Design"],
  units = "m/s²";

CREATE formulas:rocket_equation SET
  name = "Rocket Equation (Tsiolkovsky)",
  formula = "Δv = vₑ ln(m₀/mf)",
  variables = ["Δv (change in velocity)", "vₑ (exhaust velocity)", "m₀ (initial mass)", "mf (final mass)"],
  description = "Describes the relationship between rocket velocity and mass ratio.",
  applications = ["Designing Rocket Propulsion Systems", "Fuel Management"],
  units = "m/s";
