//! 向量数学工具函数
//!
//! 提供向量相似度计算等通用数学工具，供全项目共享使用，
//! 避免跨 crate 重复实现。

/// 零向量范数阈值：当向量范数低于此值时视为零向量，避免浮点除零
const NORM_THRESHOLD: f64 = 1e-10;

/// 计算两个 f32 向量的余弦相似度（Cosine Similarity），返回 f64 精度结果。
///
/// 公式：cos(θ) = (A·B) / (‖A‖ × ‖B‖)
///
/// - 当两向量长度不等或任一向量为空时，返回 `0.0`
/// - 当任一向量的 L2 范数低于 [`NORM_THRESHOLD`] 时，返回 `0.0`（数值稳定性保护）
/// - 内部以 `f64` 精度计算，避免 `f32` 累积误差
///
/// # Example
///
/// ```
/// use knowledge_core::math::cosine_similarity;
///
/// let a = vec![1.0_f32, 0.0, 0.0];
/// let b = vec![0.0_f32, 1.0, 0.0];
/// let sim = cosine_similarity(&a, &b);
/// assert!((sim - 0.0).abs() < 1e-6);
/// ```
#[must_use]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| f64::from(*x) * f64::from(*y))
        .sum();
    let norm_a: f64 = a
        .iter()
        .map(|x| f64::from(*x) * f64::from(*x))
        .sum::<f64>()
        .sqrt();
    let norm_b: f64 = b
        .iter()
        .map(|x| f64::from(*x) * f64::from(*x))
        .sum::<f64>()
        .sqrt();

    if norm_a < NORM_THRESHOLD || norm_b < NORM_THRESHOLD {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// 计算两个 f32 向量的欧氏距离（Euclidean Distance），返回 f64。
///
/// 返回值为负距离（`-‖A - B‖`），以适配"越大越相似"的排序语义。
///
/// - 当两向量长度不等或任一向量为空时，返回 `f64::MIN`
///
/// # Example
///
/// ```
/// use knowledge_core::math::euclidean_distance;
///
/// let a = vec![0.0_f32, 0.0];
/// let b = vec![3.0_f32, 4.0];
/// let dist = euclidean_distance(&a, &b);
/// assert!((dist - (-5.0)).abs() < 1e-6);
/// ```
#[must_use]
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return f64::MIN;
    }

    let sum: f64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| (f64::from(*x) - f64::from(*y)).powi(2))
        .sum();
    -sum.sqrt()
}

/// 计算两个 f32 向量的点积（Dot Product），返回 f64。
///
/// - 当两向量长度不等或任一向量为空时，返回 `0.0`
#[must_use]
pub fn dot_product(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    a.iter()
        .zip(b.iter())
        .map(|(x, y)| f64::from(*x) * f64::from(*y))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical_vectors() {
        let v = vec![1.0_f32, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!(
            (sim - 1.0).abs() < 1e-6,
            "相同向量相似度应为 1.0，实际: {sim}"
        );
    }

    #[test]
    fn test_cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0_f32, 0.0, 0.0];
        let b = vec![0.0_f32, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(
            (sim - 0.0).abs() < 1e-6,
            "正交向量相似度应为 0.0，实际: {sim}"
        );
    }

    #[test]
    fn test_cosine_similarity_empty_vectors() {
        assert!((cosine_similarity(&[], &[]) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_different_lengths() {
        assert!((cosine_similarity(&[1.0_f32], &[1.0_f32, 2.0]) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![0.0_f32, 0.0, 0.0];
        let b = vec![1.0_f32, 2.0, 3.0];
        assert!((cosine_similarity(&a, &b) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite_vectors() {
        let a = vec![1.0_f32, 0.0];
        let b = vec![-1.0_f32, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(
            (sim - (-1.0)).abs() < 1e-6,
            "反向向量相似度应为 -1.0，实际: {sim}"
        );
    }

    #[test]
    fn test_euclidean_distance_basic() {
        let a = vec![0.0_f32, 0.0];
        let b = vec![3.0_f32, 4.0];
        let dist = euclidean_distance(&a, &b);
        assert!(
            (dist - (-5.0)).abs() < 1e-6,
            "欧氏距离应为 -5.0，实际: {dist}"
        );
    }

    #[test]
    fn test_euclidean_distance_identical() {
        let v = vec![1.0_f32, 2.0, 3.0];
        let dist = euclidean_distance(&v, &v);
        assert!(
            (dist - 0.0).abs() < 1e-6,
            "相同向量距离应为 0.0，实际: {dist}"
        );
    }

    #[test]
    fn test_dot_product_basic() {
        let a = vec![1.0_f32, 2.0, 3.0];
        let b = vec![4.0_f32, 5.0, 6.0];
        let dp = dot_product(&a, &b);
        assert!((dp - 32.0).abs() < 1e-6, "点积应为 32.0，实际: {dp}");
    }

    #[test]
    fn test_dot_product_empty() {
        assert!((dot_product(&[], &[]) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_euclidean_distance_different_lengths() {
        let a = vec![1.0_f32];
        let b = vec![1.0_f32, 2.0];
        let result = euclidean_distance(&a, &b);
        assert!((result - f64::MIN).abs() < f64::EPSILON);
    }

    #[test]
    fn test_euclidean_distance_empty() {
        let result = euclidean_distance(&[], &[]);
        assert!((result - f64::MIN).abs() < f64::EPSILON);
    }

    #[test]
    fn test_dot_product_different_lengths() {
        assert!((dot_product(&[1.0_f32], &[1.0_f32, 2.0]) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_near_zero_norm() {
        let a = vec![1e-11_f32, 0.0];
        let b = vec![1.0_f32, 0.0];
        assert!((cosine_similarity(&a, &b) - 0.0).abs() < 1e-6);
    }
}
