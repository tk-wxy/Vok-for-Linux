//! 垂直卡片动态整除防截断算法与三栏几何计算模块
//!
//! 遵循 [R9 卡片高度整除算法] 与架构规范 §4.5 / §4.6。

/// 卡片垂直布局约束配置
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutConstraints {
    /// 容器上下基础外边距 (像素)
    pub padding_y: u32,
    /// 卡片行与行之间的间距 (像素)
    pub gap_y: u32,
    /// 目标期望单行卡片高度 (像素，如 88px)
    pub target_row_height: u32,
    /// 单行卡片允许的最小高度 (像素)
    pub min_row_height: u32,
    /// 单行卡片允许的最大高度 (像素)
    pub max_row_height: u32,
    /// 允许展示的最少行数
    pub min_rows: u32,
    /// 允许展示的最多行数
    pub max_rows: u32,
}

impl Default for LayoutConstraints {
    fn default() -> Self {
        Self {
            padding_y: 16,
            gap_y: 12,
            target_row_height: 88,
            min_row_height: 56,
            max_row_height: 140,
            min_rows: 2,
            max_rows: 32,
        }
    }
}

/// 计算得出的卡片垂直布局结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputedCardLayout {
    /// 最终采用的行数
    pub row_count: u32,
    /// 保证整除的单行卡片精确高度 (像素)
    pub row_height: u32,
    /// 顶部有效内边距 (吸收了下取整的像素余数)
    pub padding_top: u32,
    /// 底部有效内边距 (吸收了下取整的像素余数)
    pub padding_bottom: u32,
    /// 行间距 (像素)
    pub gap_y: u32,
    /// 占用的总高度 (必须恒等于传入的可用高度)
    pub total_height: u32,
}

impl ComputedCardLayout {
    /// 校验布局数学等式：顶部边距 + N行卡片 + (N-1)间距 + 底部边距 == 总可用高度
    pub fn is_exact(&self, available_height: u32) -> bool {
        if self.row_count == 0 {
            return self.padding_top + self.padding_bottom == available_height;
        }
        let total_gaps = (self.row_count - 1) * self.gap_y;
        let total_cards = self.row_count * self.row_height;
        let sum = self.padding_top + self.padding_bottom + total_cards + total_gaps;
        sum == available_height
    }
}

/// 三栏水平布局约束配置
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreePaneConstraints {
    /// 左右整体内边距
    pub padding_x: u32,
    /// 栏与栏之间的水平间距 (1:2:1 之间两处间距)
    pub pane_gap_x: u32,
    /// 单栏内部的左右内边距
    pub pane_inner_padding_x: u32,
    /// 启动台卡片列数 (固定为 4)
    pub launcher_cols: u32,
    /// 中转站卡片列数 (固定为 2)
    pub stage_cols: u32,
    /// 启动台卡片列间距
    pub launcher_card_gap_x: u32,
    /// 中转站卡片列间距
    pub stage_card_gap_x: u32,
}

impl Default for ThreePaneConstraints {
    fn default() -> Self {
        Self {
            padding_x: 24,
            pane_gap_x: 16,
            pane_inner_padding_x: 12,
            launcher_cols: 4,
            stage_cols: 2,
            launcher_card_gap_x: 8,
            stage_card_gap_x: 12,
        }
    }
}

/// 计算得出的 1:2:1 三栏几何与卡片宽度
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputedThreePaneLayout {
    /// 左栏 (启动台 25%) 宽度
    pub left_width: u32,
    /// 中栏 (中转站 50%) 宽度
    pub middle_width: u32,
    /// 右栏 (剪贴板 25%) 宽度
    pub right_width: u32,
    /// 启动台单张卡片计算宽度
    pub launcher_card_width: u32,
    /// 中转站单张卡片计算宽度
    pub stage_card_width: u32,
    /// 剪贴板单项可用宽度
    pub clipboard_card_width: u32,
    /// 占用的总水平宽度
    pub total_width: u32,
}

/// 布局计算器纯函数合集
pub struct CardLayoutMath;

impl CardLayoutMath {
    /// 计算防截断垂直卡片行高与内边距
    ///
    /// 核心算法：
    /// 1. $H_{content} = H_{avail} - 2P$；
    /// 2. 遍历可行行数 $N \in [N_{min}, N_{max}]$，求 $H_{row} = \lfloor \frac{H_{content} - (N - 1) \times G}{N} \rfloor$；
    /// 3. 选取使 $H_{row}$ 最贴近 $H_{target}$ 的最优行数；
    /// 4. 将下取整产生的像素余量 $R < N$ 均匀分配到上下 padding；
    /// 5. 确保总和恒等于 $H_{avail}$，视口底部绝无半截卡片截断。
    pub fn compute_card_layout(
        available_height: u32,
        constraints: &LayoutConstraints,
    ) -> ComputedCardLayout {
        let base_padding = constraints.padding_y;
        let min_required = 2 * base_padding + constraints.min_row_height;

        // 极端微小高度降级保护
        if available_height <= min_required {
            let row_h = available_height.saturating_sub(2 * base_padding).max(20);
            let rem = available_height.saturating_sub(row_h);
            let pt = rem / 2;
            let pb = rem - pt;
            return ComputedCardLayout {
                row_count: 1,
                row_height: row_h,
                padding_top: pt,
                padding_bottom: pb,
                gap_y: constraints.gap_y,
                total_height: available_height,
            };
        }

        let content_height = available_height - 2 * base_padding;

        // 寻找最优行数 N
        let mut best_n = constraints.min_rows;
        let mut best_row_h = constraints.target_row_height;
        let mut min_diff = i64::MAX;

        for n in constraints.min_rows..=constraints.max_rows {
            let total_gaps = (n - 1) * constraints.gap_y;
            if content_height <= total_gaps {
                break;
            }
            let raw_row_h = (content_height - total_gaps) / n;

            // 如果高度已低于极小值，后续更大的 N 只会更小，跳出循环
            if raw_row_h < constraints.min_row_height && n > constraints.min_rows {
                break;
            }

            let diff = (raw_row_h as i64 - constraints.target_row_height as i64).abs();
            if diff < min_diff {
                min_diff = diff;
                best_n = n;
                best_row_h = raw_row_h;
            }
        }

        // 计算卡片占用总高与余量
        let total_gaps = (best_n - 1) * constraints.gap_y;
        let cards_total = best_n * best_row_h + total_gaps;
        let remainder = content_height.saturating_sub(cards_total);

        // 将余量均匀吸收进上下边距
        let pad_top = base_padding + remainder / 2;
        let pad_bottom = base_padding + (remainder - remainder / 2);

        ComputedCardLayout {
            row_count: best_n,
            row_height: best_row_h,
            padding_top: pad_top,
            padding_bottom: pad_bottom,
            gap_y: constraints.gap_y,
            total_height: available_height,
        }
    }

    /// 计算 1:2:1 三栏水平几何与卡片列宽
    ///
    /// 宽度分配比例：左启动台 25% (1份)，中间中转站 50% (2份)，右剪贴板 25% (1份)。
    /// 像素整除余量由核心视觉区（中转站）吸收，保障像素总和绝对闭合。
    pub fn compute_three_pane_layout(
        available_width: u32,
        constraints: &ThreePaneConstraints,
    ) -> ComputedThreePaneLayout {
        let outer_gaps = 2 * constraints.padding_x + 2 * constraints.pane_gap_x;
        if available_width <= outer_gaps + 100 {
            // 极小视口降级保底
            let w_each = available_width / 3;
            return ComputedThreePaneLayout {
                left_width: w_each,
                middle_width: w_each,
                right_width: available_width.saturating_sub(2 * w_each),
                launcher_card_width: 30,
                stage_card_width: 50,
                clipboard_card_width: 50,
                total_width: available_width,
            };
        }

        let net_width = available_width - outer_gaps;
        let unit = net_width / 4;
        let remainder = net_width % 4;

        let left_width = unit;
        // 中间中转站占 2 份并吸收微小像素余量
        let middle_width = 2 * unit + remainder;
        let right_width = unit;

        // 计算启动台卡片宽度 (固定 4 列)
        let launcher_inner = left_width.saturating_sub(2 * constraints.pane_inner_padding_x);
        let launcher_gaps = (constraints.launcher_cols - 1) * constraints.launcher_card_gap_x;
        let launcher_card_width = if launcher_inner > launcher_gaps {
            (launcher_inner - launcher_gaps) / constraints.launcher_cols
        } else {
            30
        };

        // 计算中转站卡片宽度 (固定 2 列)
        let stage_inner = middle_width.saturating_sub(2 * constraints.pane_inner_padding_x);
        let stage_gaps = (constraints.stage_cols - 1) * constraints.stage_card_gap_x;
        let stage_card_width = if stage_inner > stage_gaps {
            (stage_inner - stage_gaps) / constraints.stage_cols
        } else {
            50
        };

        // 计算剪贴板单项可用宽度
        let clipboard_card_width = right_width.saturating_sub(2 * constraints.pane_inner_padding_x);

        ComputedThreePaneLayout {
            left_width,
            middle_width,
            right_width,
            launcher_card_width,
            stage_card_width,
            clipboard_card_width,
            total_width: available_width,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_card_layout_exact_division_no_clip_1080p() {
        let constraints = LayoutConstraints::default();
        // 1080p 工作区高度示例：减去任务栏和顶部搜索区后，假设卡片可视区高度为 780px
        let height = 780;
        let layout = CardLayoutMath::compute_card_layout(height, &constraints);

        assert!(layout.is_exact(height), "布局总和必须与可用高度完全一致");
        assert!(layout.row_count >= constraints.min_rows);
        assert!(layout.row_count <= constraints.max_rows);
        assert!(layout.row_height >= constraints.min_row_height);
        assert!(layout.row_height <= constraints.max_row_height);

        // 验证余数吸收后未发生像素溢出
        let total_cards_h = layout.row_count * layout.row_height + (layout.row_count - 1) * layout.gap_y;
        assert_eq!(
            layout.padding_top + total_cards_h + layout.padding_bottom,
            height
        );
    }

    #[test]
    fn test_card_layout_multi_resolutions_exactness() {
        let constraints = LayoutConstraints::default();
        // 遍历不同桌面分辨率与任意奇数高度
        let test_heights = [
            480,  // 较小窗口 / 720p 屏幕可用高
            625,  // 奇数高度
            780,  // 1080p 典型工作台卡片区高
            953,  // 2K / 1440p 奇数高度
            1240, // 2K 全屏高
            1823, // 4K 奇数工作区高
        ];

        for &h in &test_heights {
            let layout = CardLayoutMath::compute_card_layout(h, &constraints);
            assert!(
                layout.is_exact(h),
                "高度 {} 下的计算结果未精确闭合: {:?}",
                h,
                layout
            );
            // 确保不会切掉最后一行卡片 (第 N+1 行完全溢出)
            let n = layout.row_count;
            let current_content_h = n * layout.row_height + (n - 1) * layout.gap_y;
            let next_row_needed_h = current_content_h + layout.gap_y + layout.row_height;
            assert!(
                next_row_needed_h > (h - layout.padding_top - layout.padding_bottom),
                "第 N+1 行绝不允许露出任何部分"
            );
        }
    }

    #[test]
    fn test_card_layout_extreme_small_height() {
        let constraints = LayoutConstraints::default();
        // 极小高度 (如只有 80px)
        let layout = CardLayoutMath::compute_card_layout(80, &constraints);
        assert!(layout.row_count >= 1);
        assert_eq!(layout.total_height, 80);
    }

    #[test]
    fn test_three_pane_horizontal_split_1_2_1() {
        let constraints = ThreePaneConstraints::default();
        let total_width = 1920; // 1080p 宽度
        let layout = CardLayoutMath::compute_three_pane_layout(total_width, &constraints);

        // 验证三栏与所有间隙之和严格等于总宽度
        let sum = layout.left_width
            + layout.middle_width
            + layout.right_width
            + 2 * constraints.padding_x
            + 2 * constraints.pane_gap_x;
        assert_eq!(sum, total_width);

        // 验证 1:2:1 比例关系 (中间宽度近似等于左栏与右栏之和)
        assert_eq!(layout.left_width, layout.right_width);
        assert!(layout.middle_width >= 2 * layout.left_width);

        // 验证启动台 4 列卡片宽度大于 0
        assert!(layout.launcher_card_width > 50);
        // 验证中转站 2 列大卡片宽度大于启动台单卡宽度
        assert!(layout.stage_card_width > layout.launcher_card_width);
    }

    #[test]
    fn test_three_pane_odd_width_pixel_absorption() {
        let constraints = ThreePaneConstraints::default();
        let odd_widths = [1365, 1441, 1600, 2561, 3840];
        for &w in &odd_widths {
            let layout = CardLayoutMath::compute_three_pane_layout(w, &constraints);
            let sum = layout.left_width
                + layout.middle_width
                + layout.right_width
                + 2 * constraints.padding_x
                + 2 * constraints.pane_gap_x;
            assert_eq!(sum, w, "宽度 {} 的三栏求和不匹配", w);
        }
    }
}
