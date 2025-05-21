#!/bin/bash

echo "运行Nostr-CRDT基准测试..."
echo "========================="

# 设置环境变量以控制Criterion输出
export CRITERION_DEBUG=0

# 检查是否安装了 hyperfine
if command -v hyperfine &> /dev/null; then
    echo "使用hyperfine运行基础性能测试"
    hyperfine 'cargo bench --bench crdt_benchmark -- --measurement-time 2 --sample-size 10 "LWWRegister/single_update"' \
              'cargo bench --bench crdt_benchmark -- --measurement-time 2 --sample-size 10 "GCounter/single_increment"' \
              'cargo bench --bench crdt_benchmark -- --measurement-time 2 --sample-size 10 "GSet/single_add"'
    echo ""
fi

# 运行所有基准测试
echo "运行完整基准测试套件..."
cargo bench

echo ""
echo "基准测试完成！"
echo "结果保存在 target/criterion/ 目录下"
echo "可以在浏览器中打开HTML报告查看详细结果" 