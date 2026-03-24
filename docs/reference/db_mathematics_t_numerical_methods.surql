USE NS project_universe;
USE DB mathematics;

DEFINE TABLE numerical_methods SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE numerical_methods TYPE string;
DEFINE FIELD description     ON TABLE numerical_methods TYPE string;
DEFINE FIELD formula         ON TABLE numerical_methods TYPE string;
DEFINE FIELD variables       ON TABLE numerical_methods TYPE array;
DEFINE FIELD applications    ON TABLE numerical_methods TYPE array;
DEFINE FIELD advantages      ON TABLE numerical_methods TYPE array;
DEFINE FIELD limitations     ON TABLE numerical_methods TYPE array;

DEFINE INDEX unique_method ON TABLE numerical_methods COLUMNS name UNIQUE;

CREATE numerical_methods:euler SET
  name = "Euler Method",
  description = "First-order numerical procedure for solving ordinary differential equations.",
  formula = "yₙ₊₁ = yₙ + hf(tₙ, yₙ)",
  variables = ["yₙ₊₁ (next value)", "yₙ (current value)", "h (step size)", "f(tₙ, yₙ) (derivative)"],
  applications = ["Basic Trajectory Simulations", "Initial Course Plotting"],
  advantages = ["Simple to implement", "Computationally efficient"],
  limitations = ["Low accuracy", "Error accumulation"];

CREATE numerical_methods:runge_kutta SET
  name = "Runge-Kutta Method",
  description = "Family of iterative methods for approximating solutions of ordinary differential equations.",
  formula = "yₙ₊₁ = yₙ + (k₁ + 2k₂ + 2k₃ + k₄)/6",
  variables = ["yₙ₊₁ (next value)", "yₙ (current value)", "k₁,k₂,k₃,k₄ (slope estimates)"],
  applications = ["Precise Orbital Calculations", "Advanced Trajectory Planning"],
  advantages = ["High accuracy", "Stable"],
  limitations = ["More complex", "Computationally intensive"];

CREATE numerical_methods:newton_raphson SET
  name = "Newton-Raphson Method",
  description = "Iterative method for finding successively better approximations to the roots of a real-valued function.",
  formula = "xₙ₊₁ = xₙ - f(xₙ)/f'(xₙ)",
  variables = ["xₙ₊₁ (next approximation)", "xₙ (current approximation)", "f(xₙ) (function value)", "f'(xₙ) (derivative)"],
  applications = ["Solving Navigation Equations", "Precise Targeting"],
  advantages = ["Fast convergence", "High accuracy"],
  limitations = ["Requires derivative", "May not converge"];

CREATE numerical_methods:monte_carlo SET
  name = "Monte Carlo Simulation",
  description = "Computational algorithm that relies on repeated random sampling to obtain numerical results.",
  formula = "Result = Average of random samples",
  variables = ["Number of samples", "Random variables", "Probability distributions"],
  applications = ["Probabilistic Events", "Asteroid Resource Estimation", "Mission Risk Analysis"],
  advantages = ["Handles complex systems", "Probabilistic results"],
  limitations = ["Computationally expensive", "Statistical uncertainty"];

CREATE numerical_methods:finite_difference SET
  name = "Finite Difference Method",
  description = "Numerical method for solving differential equations by approximating derivatives with finite differences.",
  formula = "f'(x) ≈ (f(x+h) - f(x))/h",
  variables = ["f(x) (function)", "h (step size)", "x (point)"],
  applications = ["Thermal Management", "Fluid Dynamics Simulation"],
  advantages = ["Simple to implement", "Direct physical interpretation"],
  limitations = ["Grid dependent", "Accuracy issues at boundaries"];

CREATE numerical_methods:gradient_descent SET
  name = "Gradient Descent",
  description = "First-order iterative optimization algorithm for finding a local minimum of a differentiable function.",
  formula = "xₙ₊₁ = xₙ - α∇f(xₙ)",
  variables = ["xₙ₊₁ (next point)", "xₙ (current point)", "α (learning rate)", "∇f(xₙ) (gradient)"],
  applications = ["AI Optimization", "Adaptive Autopilots", "Resource Optimization"],
  advantages = ["Simple", "Works with high dimensions"],
  limitations = ["May get stuck in local minima", "Sensitive to learning rate"];

CREATE numerical_methods:gauss_seidel SET
  name = "Gauss-Seidel Method",
  description = "Iterative method for solving a system of linear equations.",
  formula = "xᵢ⁽ᵏ⁺¹⁾ = (bᵢ - Σⱼ₌₁ⁱ⁻¹aᵢⱼxⱼ⁽ᵏ⁺¹⁾ - Σⱼ₌ᵢ₊₁ⁿaᵢⱼxⱼ⁽ᵏ⁾)/aᵢᵢ",
  variables = ["xᵢ (unknowns)", "aᵢⱼ (coefficients)", "bᵢ (constants)"],
  applications = ["Engineering Simulations", "Electrical System Analysis", "Thermal Control Systems"],
  advantages = ["Simple to implement", "Memory efficient"],
  limitations = ["May not converge", "Order dependent"];
