import rnbt as rnbt # This should match the name of your .so or .pyd file.
#import logging

#logging.basicConfig(level=logging.INFO)
#mc_binary = rnbt.load_binary('tests/resources/test_world/r.-1.0.mca')
mc_binary = rnbt.load_binary('C:/MultiMC/MultiMC/instances/Fabulously.Optimized.MC.1.20.1.auto-update/.minecraft/saves/fast-nbt test/')
redstone = mc_binary.search_blocks(['minecraft:repeater', 'minecraft:lever', 'minecraft:iron_block', 'minecraft:piston'])

print('Repeater Coords')
print(redstone['minecraft:repeater'][0].coord.x)
print(redstone['minecraft:repeater'][0].coord.y)
print(redstone['minecraft:repeater'][0].coord.z)

print('Repeater Properties')
print(redstone['minecraft:repeater'][0].properties)

# tag_found, blocks_list_tag = mc_binary.search_compound('block_states')

# if tag_found:
#     for block_tag in blocks_list_tag:
#         print(block_tag)
# else:
#     print('Tag not found')
    