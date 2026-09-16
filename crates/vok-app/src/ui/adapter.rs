use gtk4::prelude::*;
use vok_core::layout::{
    CardLayoutMath, ComputedCardLayout, ComputedThreePaneLayout, LayoutConstraints,
    ThreePaneConstraints,
};

/// GTK 容器尺寸与纯数学布局整除算法之间的适配器
pub struct LayoutAdapter;

impl LayoutAdapter {
    /// 计算并应用三栏水平几何与卡片防截断垂直高度
    pub fn apply_layout(
        available_width: u32,
        available_height: u32,
        launcher_pane: &impl IsA<gtk4::Widget>,
        stage_pane: &impl IsA<gtk4::Widget>,
        clipboard_pane: &impl IsA<gtk4::Widget>,
    ) -> (ComputedThreePaneLayout, ComputedCardLayout) {
        let three_pane_constraints = ThreePaneConstraints::default();
        let three_pane = CardLayoutMath::compute_three_pane_layout(
            available_width,
            &three_pane_constraints,
        );

        // 设置左（启动台 25%）、中（中转站 50%）、右（剪贴板 25%）的精确宽度约束
        launcher_pane.set_size_request(three_pane.left_width as i32, -1);
        stage_pane.set_size_request(three_pane.middle_width as i32, -1);
        clipboard_pane.set_size_request(three_pane.right_width as i32, -1);

        // 计算卡片防截断垂直高度约束
        let layout_constraints = LayoutConstraints::default();
        let card_layout = CardLayoutMath::compute_card_layout(
            available_height,
            &layout_constraints,
        );

        (three_pane, card_layout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_layout_math() {
        let tp = CardLayoutMath::compute_three_pane_layout(1920, &ThreePaneConstraints::default());
        assert!(tp.left_width > 0);
        assert!(tp.middle_width > tp.left_width);

        let cl = CardLayoutMath::compute_card_layout(800, &LayoutConstraints::default());
        assert!(cl.is_exact(800));
    }
}
