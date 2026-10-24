using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace D2SLib2.Structure
{
    public static class HeaderOffsets
    {                                                             //                           byte Offset      Length in bits
        public static readonly OffsetStruct OFFSET_SIGNATURE = new OffsetStruct                         (0,     32);
        public static readonly OffsetStruct OFFSET_VERSION = new OffsetStruct                           (4,     32);
        public static readonly OffsetStruct OFFSET_FILESIZE = new OffsetStruct                          (8,     32);
        public static readonly OffsetStruct OFFSET_CHECKSUM = new OffsetStruct                          (12,    32);
    }

    public static class PlayerInformationOffsets
    {                                                             //                           byte Offset      Length in bits
        public static readonly OffsetStruct OFFSET_ACTIVE_WEAPON = new OffsetStruct                     (16,    32);
        public static readonly OffsetStruct OFFSET_CHARACTER_STATUS = new OffsetStruct                  (36,    8);
        public static readonly OffsetStruct OFFSET_PROGRESSION = new OffsetStruct                       (37,    8);
        public static readonly OffsetStruct OFFSET_PLAYERCLASS = new OffsetStruct                       (40,    8);
        public static readonly OffsetStruct OFFSET_PLAYERLEVEL = new OffsetStruct                       (43,    8);
        public static readonly OffsetStruct OFFSET_UNIX_TIMESTAMP = new OffsetStruct                    (48,    32);
        public static readonly OffsetStruct OFFSET_ASSIGNED_SKILLS = new OffsetStruct                   (56,    512);
        public static readonly OffsetStruct OFFSET_LEFTMOUSESKILL = new OffsetStruct                    (120,   32);
        public static readonly OffsetStruct OFFSET_RIGHTMOUSESKILL = new OffsetStruct                   (124,   32);
        public static readonly OffsetStruct OFFSET_SWITCHLEFTMOUSESKILL = new OffsetStruct              (128,   32);
        public static readonly OffsetStruct OFFSET_SWITCHRIGHTMOUSESKILL = new OffsetStruct             (132,   32);
        public static readonly OffsetStruct OFFSET_DIFFICULTY = new OffsetStruct                        (168,   24);
        public static readonly OffsetStruct OFFSET_MAPID = new OffsetStruct                             (171,   32);
        public static readonly OffsetStruct OFFSET_PLAYER_NAME = new OffsetStruct                       (267,   128);
        public static readonly OffsetStruct OFFSET_CORPSE_INFORMATION_MARKER = new OffsetStruct         (-1,    16, signature: "JM");
        public static readonly OffsetStruct OFFSET_CORPSE_COUNT = new OffsetStruct                      (-1,    16);
        public static readonly OffsetStruct OFFSET_CORPSE_XY_LOCATION = new OffsetStruct                (-1,    32);
        public static readonly OffsetStruct OFFSET_MERCENARY_INVENTORY_MARKER = new OffsetStruct        (-1,    16, signature: "jf");
        public static readonly OffsetStruct OFFSET_IRON_GOLEM_MARKER = new OffsetStruct                 (-1,    16, signature: "kf");
        public static readonly OffsetStruct OFFSET_IRON_GOLEM_BOOL = new OffsetStruct                   (-1,    8);
    }

    public static class OtherOffsets
    {                                                             //                           byte Offset      Length in bits
        public static readonly OffsetStruct OFFSET_CHARACTER_MENU_APPEARANCE = new OffsetStruct         (136,   256);
        public static readonly OffsetStruct OFFSET_D2R_CHARACTER_MENU_APPEARANCE = new OffsetStruct     (219,   384);

        public static readonly OffsetStruct OFFSET_MERCENARY_IS_DEAD = new OffsetStruct                 (177,   16);
        public static readonly OffsetStruct OFFSET_MERCENARY_SEED = new OffsetStruct                    (179,   32);
        public static readonly OffsetStruct OFFSET_MERCENARY_NAMEID = new OffsetStruct                  (183,   16);
        public static readonly OffsetStruct OFFSET_MERCENARY_TYPE = new OffsetStruct                    (185,   16);
        public static readonly OffsetStruct OFFSET_MERCENARY_EXPERIENCE = new OffsetStruct              (187,   32);
        
        public static readonly OffsetStruct OFFSET_NPC_MARKER = new OffsetStruct                        (713,    16);
        public static readonly OffsetStruct OFFSET_NPC_SIZE = new OffsetStruct                          (715,    16);
        public static readonly OffsetStruct OFFSET_NPC_INTRO_NORMAL = new OffsetStruct                  (717,    64);
        public static readonly OffsetStruct OFFSET_NPC_INTRO_NIGHTMARE = new OffsetStruct               (725,    64);
        public static readonly OffsetStruct OFFSET_NPC_INTRO_HELL = new OffsetStruct                    (733,    64);
        public static readonly OffsetStruct OFFSET_NPC_CONGRATS_NORMAL = new OffsetStruct               (741,    64);
        public static readonly OffsetStruct OFFSET_NPC_CONGRATS_NIGHTMARE = new OffsetStruct            (749,    64);
        public static readonly OffsetStruct OFFSET_NPC_CONGRATS_HELL = new OffsetStruct                 (757,    64);
    }

    public static class QuestOffsets
    {                                                             //                           byte Offset      Length in bits
        public static readonly OffsetStruct OFFSET_SIGNATURE = new OffsetStruct                         (335,   32, signature: "Woo!");
        public static readonly OffsetStruct OFFSET_VERSION = new OffsetStruct                           (339,   32);
        public static readonly OffsetStruct OFFSET_SIZE = new OffsetStruct                              (343,   16);
        
        public static readonly OffsetStruct OFFSET_NORMAL = new OffsetStruct                            (345,   768);
        public static readonly OffsetStruct OFFSET_NIGHTMARE = new OffsetStruct                         (441,   768);
        public static readonly OffsetStruct OFFSET_HELL = new OffsetStruct                              (537,   768);

        // The following relies on Offset addition adding the difficulty offset and
        //  the desired act or other flags then if you want quests, you must add 
        //  Difficulty + Act + Quest(1-6) which give you the correct offset
        public static readonly OffsetStruct OFFSET_INTRODUCTION_WARRIV = new OffsetStruct               (0,     16);
        public static readonly OffsetStruct OFFSET_TRAVELED_ACT2 = new OffsetStruct                     (14,    16);
        public static readonly OffsetStruct OFFSET_INTRODUCTION_JERHYN = new OffsetStruct               (16,    16);
        public static readonly OffsetStruct OFFSET_TRAVELED_ACT3 = new OffsetStruct                     (30,    16);
        public static readonly OffsetStruct OFFSET_INTRODUCTION_HRATLI = new OffsetStruct               (32,    16);
        public static readonly OffsetStruct OFFSET_TRAVELED_ACT4 = new OffsetStruct                     (46,    16);
        public static readonly OffsetStruct OFFSET_INTRODUCTION_TYERAL = new OffsetStruct               (48,    16);
        public static readonly OffsetStruct OFFSET_TRAVELED_ACT5 = new OffsetStruct                     (62,    16);
        public static readonly OffsetStruct OFFSET_INTRODUCTION_CAIN = new OffsetStruct                 (64,    16);
        public static readonly OffsetStruct OFFSET_AKARA_STAT_RESET = new OffsetStruct                  (82,    8);
        public static readonly OffsetStruct OFFSET_ACT5_COMPLETED = new OffsetStruct                    (83,    8);

        public static readonly OffsetStruct OFFSET_ACT1 = new OffsetStruct                              (2,     96);
        public static readonly OffsetStruct OFFSET_ACT2 = new OffsetStruct                              (18,    96);
        public static readonly OffsetStruct OFFSET_ACT3 = new OffsetStruct                              (34,    96);
        public static readonly OffsetStruct OFFSET_ACT4 = new OffsetStruct                              (50,    48);
        public static readonly OffsetStruct OFFSET_ACT5 = new OffsetStruct                              (70,    96);

        public static readonly OffsetStruct OFFSET_QUEST1 = new OffsetStruct                            (0,     16);
        public static readonly OffsetStruct OFFSET_QUEST2 = new OffsetStruct                            (2,     16);
        public static readonly OffsetStruct OFFSET_QUEST3 = new OffsetStruct                            (4,     16);
        public static readonly OffsetStruct OFFSET_QUEST4 = new OffsetStruct                            (6,     16);
        public static readonly OffsetStruct OFFSET_QUEST5 = new OffsetStruct                            (8,     16);
        public static readonly OffsetStruct OFFSET_QUEST6 = new OffsetStruct                            (10,    16);
        
        public static readonly int ACT1_QUEST_COUNT =                                                   6;
        public static readonly int ACT2_QUEST_COUNT =                                                   6;
        public static readonly int ACT3_QUEST_COUNT =                                                   6;
        public static readonly int ACT4_QUEST_COUNT =                                                   3;
        public static readonly int ACT5_QUEST_COUNT =                                                   6;
    }

    public static class WaypointOffsets
    {                                                             //                           byte Offset      Length in bits
        public static readonly OffsetStruct OFFSET_SIGNATURE = new OffsetStruct                         (633,   16, signature: "WS");
        public static readonly OffsetStruct OFFSET_VERSION = new OffsetStruct                           (635,   32);
        public static readonly OffsetStruct OFFSET_SIZE = new OffsetStruct                              (639,   16);
        
        public static readonly OffsetStruct OFFSET_NORMAL = new OffsetStruct                            (641,   16);
        public static readonly OffsetStruct OFFSET_NIGHTMARE = new OffsetStruct                         (665,   16);
        public static readonly OffsetStruct OFFSET_HELL = new OffsetStruct                              (689,   16);

        // The following relies on Offset addition adding the difficulty offset and
        //  the desired act, you must add Difficulty + Act + (n)Waypoints which give
        //  you the correct offset
        public static readonly OffsetStruct OFFSET_ACT1 = new OffsetStruct                              (2,     8);
        public static readonly OffsetStruct OFFSET_ACT2 = new OffsetStruct                              (3,     8);
        public static readonly OffsetStruct OFFSET_ACT3 = new OffsetStruct                              (4,     8);
        public static readonly OffsetStruct OFFSET_ACT4 = new OffsetStruct                              (5,     8);
        public static readonly OffsetStruct OFFSET_ACT5 = new OffsetStruct                              (6,     8);

        public static readonly int ACT1_WAYPOINT_COUNT =                                                8;
        public static readonly int ACT2_WAYPOINT_COUNT =                                                8;
        public static readonly int ACT3_WAYPOINT_COUNT =                                                8;
        public static readonly int ACT4_WAYPOINT_COUNT =                                                3;
        public static readonly int ACT5_WAYPOINT_COUNT =                                                8;
    }

    public static class AttributeOffsets
    {
        public static readonly OffsetStruct OFFSET_SIGNATURE = new OffsetStruct                         (765,   16, signature: "gf");
    }

    public static class SkillsOffsets
    {
        public static readonly OffsetStruct OFFSET_START_SEARCH = new OffsetStruct                      (765,   16, signature: "if");
        public static readonly OffsetStruct OFFSET_SKILL = new OffsetStruct                             (-1,    8);
    }

    public static class InventoryOffsets
    {
        public static readonly OffsetStruct OFFSET_START_SEARCH = new OffsetStruct                      (765,   16, signature: "JM");
        
        // This section of bits/bytes must be read in succession as each item is not searchable
        public static readonly ItemOffsetStruct OFFSET_ITEM_COUNT = new ItemOffsetStruct                (-1,    16);
        public static readonly ItemOffsetStruct OFFSET_BASIC_ITEM_FLAGS = new ItemOffsetStruct          (-1,    32);
        public static readonly ItemOffsetStruct OFFSET_ITEM_VERSION = new ItemOffsetStruct              (-1,    3);
        public static readonly ItemOffsetStruct OFFSET_PARENT_LOCATION = new ItemOffsetStruct           (-1,    3);
        public static readonly ItemOffsetStruct OFFSET_EQUIPPED_LOCATION = new ItemOffsetStruct         (-1,    4);
        public static readonly ItemOffsetStruct OFFSET_INV_X_POSITION = new ItemOffsetStruct            (-1,    4);
        public static readonly ItemOffsetStruct OFFSET_INV_Y_POSITION = new ItemOffsetStruct            (-1,    4);
        public static readonly ItemOffsetStruct OFFSET_STORED_LOCATION = new ItemOffsetStruct           (-1,    3);
        public static readonly ItemOffsetStruct OFFSET_CODE = new ItemOffsetStruct                      (-1,    32);
        public static readonly ItemOffsetStruct OFFSET_SIMPLE_SOCKETED_COUNT = new ItemOffsetStruct     (-1,    1);
        public static readonly ItemOffsetStruct OFFSET_SOCKETED_COUNT = new ItemOffsetStruct            (-1,    3);
        public static readonly ItemOffsetStruct OFFSET_ID = new ItemOffsetStruct                        (-1,    32);
        public static readonly ItemOffsetStruct OFFSET_ITEM_LEVEL = new ItemOffsetStruct                (-1,    7);
        public static readonly ItemOffsetStruct OFFSET_QUALITY = new ItemOffsetStruct                   (-1,    4);
        public static readonly ItemOffsetStruct OFFSET_HAS_MULTIPLE_GRAPHICS = new ItemOffsetStruct     (-1,    1);
        public static readonly ItemOffsetStruct OFFSET_MULTIPLE_GRAPHICS = new ItemOffsetStruct         (-1,    3);
        public static readonly ItemOffsetStruct OFFSET_HAS_AUTO_AFFIX = new ItemOffsetStruct            (-1,    1);
        public static readonly ItemOffsetStruct OFFSET_AUTO_AFFIX = new ItemOffsetStruct                (-1,    11);
        public static readonly ItemOffsetStruct OFFSET_FILE_INDEX_INFERIOR = new ItemOffsetStruct       (-1,    3);
        public static readonly ItemOffsetStruct OFFSET_FILE_INDEX_SUPERIOR = new ItemOffsetStruct       (-1,    3);
        public static readonly ItemOffsetStruct OFFSET_MAGIC_PREFIX_ID = new ItemOffsetStruct           (-1,    11);
        public static readonly ItemOffsetStruct OFFSET_MAGIC_SUFFIX_ID = new ItemOffsetStruct           (-1,    11);
        public static readonly ItemOffsetStruct OFFSET_RARE_PREFIX_ID = new ItemOffsetStruct            (-1,    8);
        public static readonly ItemOffsetStruct OFFSET_RARE_SUFFIX_ID = new ItemOffsetStruct            (-1,    8);
        public static readonly ItemOffsetStruct OFFSET_CRAFT_PREFIX_ID = new ItemOffsetStruct           (-1,    8);
        public static readonly ItemOffsetStruct OFFSET_CRAFT_SUFFIX_ID = new ItemOffsetStruct           (-1,    8);
        public static readonly ItemOffsetStruct OFFSET_RARE_PREFIX_VALUE_BOOL = new ItemOffsetStruct    (-1,    1);
        public static readonly ItemOffsetStruct OFFSET_RARE_PREFIX_VALUE = new ItemOffsetStruct         (-1,    11);
        public static readonly ItemOffsetStruct OFFSET_RARE_SUFFIX_VALUE_BOOL = new ItemOffsetStruct    (-1,    1);
        public static readonly ItemOffsetStruct OFFSET_RARE_SUFFIX_VALUE = new ItemOffsetStruct         (-1,    11);
        public static readonly ItemOffsetStruct OFFSET_CRAFT_PREFIX_VALUE_BOOL =new ItemOffsetStruct    (-1,    1);
        public static readonly ItemOffsetStruct OFFSET_CRAFT_PREFIX_VALUE = new ItemOffsetStruct        (-1,    11);
        public static readonly ItemOffsetStruct OFFSET_CRAFT_SUFFIX_VALUE_BOOL =new ItemOffsetStruct    (-1,    1);
        public static readonly ItemOffsetStruct OFFSET_CRAFT_SUFFIX_VALUE = new ItemOffsetStruct        (-1,    11);
        public static readonly ItemOffsetStruct OFFSET_SET_FILE_INDEX = new ItemOffsetStruct            (-1,    12);
        public static readonly ItemOffsetStruct OFFSET_UNIQUE_FILE_INDEX = new ItemOffsetStruct         (-1,    12);
        public static readonly ItemOffsetStruct OFFSET_RUNEWORD_ID = new ItemOffsetStruct               (-1,    12);
        public static readonly ItemOffsetStruct OFFSET_RUNEWORD_PROPERTY = new ItemOffsetStruct         (-1,    4);
        public static readonly ItemOffsetStruct OFFSET_PERSONALIZED_NAME_SECTION = new ItemOffsetStruct (-1,    128);
        public static readonly ItemOffsetStruct OFFSET_PERSONALIZED_CHAR = new ItemOffsetStruct         (-1,    8);
        public static readonly ItemOffsetStruct OFFSET_TOWN_PORTAL_BOOK = new ItemOffsetStruct          (-1,    5, "tbk");
        public static readonly ItemOffsetStruct OFFSET_IDENTIFY_BOOK = new ItemOffsetStruct             (-1,    5, "ibk");
        public static readonly ItemOffsetStruct OFFSET_REALM_DATA_BOOL = new ItemOffsetStruct           (-1,    1);
        public static readonly ItemOffsetStruct OFFSET_REALM_DATA = new ItemOffsetStruct                (-1,    96);
        public static readonly ItemOffsetStruct OFFSET_ARMOR_VALUE = new ItemOffsetStruct               (-1,    11);
        public static readonly ItemOffsetStruct OFFSET_QUANTITY = new ItemOffsetStruct                  (-1,    9);
        public static readonly ItemOffsetStruct OFFSET_SOCKET_COUNT = new ItemOffsetStruct              (-1,    4);
        public static readonly ItemOffsetStruct OFFSET_SET_ITEM_MASK = new ItemOffsetStruct             (-1,    5);
        public static readonly ItemOffsetStruct OFFSET_EAR_FILE_INDEX = new ItemOffsetStruct            (-1,    3);
        public static readonly ItemOffsetStruct OFFSET_EAR_LEVEL = new ItemOffsetStruct                 (-1,    7);
        public static readonly ItemOffsetStruct OFFSET_MAGICAL_ATTRIABUTE_ID = new ItemOffsetStruct     (-1,    9);
    }

    [DebuggerDisplay("{Offset}, {BitLength}, {ByteLength}")]
    public struct OffsetStruct
    {
        public int Offset { get; }
        public int BitOffset { get; }
        public int ByteLength { get; }
        public int BitLength { get; }
        public string Signature { get; }
        public bool isUsingBitOffset = false;

        public OffsetStruct(int offset, int bitLength, bool isBitOffsetEnabled = false, string signature = "")
        {
            isUsingBitOffset = isBitOffsetEnabled;
            if (isUsingBitOffset) { BitOffset = offset; Offset = -1;  }
            else { Offset = offset; BitOffset = -1; }
            BitLength = bitLength;
            ByteLength = bitLength / 8;
            Signature = signature;
        }
    }
    
    public struct ItemOffsetStruct
    {
        public int BitOffset { get; }
        public int BitLength { get; }
        public string ItemCode { get; }
        
        public ItemOffsetStruct(int bitOffset, int bitLength, string itemCode = "")
        {
            BitOffset = bitOffset;
            BitLength = bitLength;
            ItemCode = itemCode;
        }
    }
}
