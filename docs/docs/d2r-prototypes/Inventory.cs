using D2SLib2.BinaryHandler;
using Microsoft.Extensions.Logging;
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace D2SLib2.Structure.Player.Item
{
    public class Inventory
    {
        public UInt16 ItemCount = 0;
        public HashSet<ItemStructure> ItemList = new HashSet<ItemStructure>();

        public Inventory() { }

        public static Inventory Read(BitwiseBinaryReader mainReader)
        {
            Inventory inventory = new Inventory();

            if (mainReader.ReadSkipPositioning<string>(InventoryOffsets.OFFSET_START_SEARCH) != InventoryOffsets.OFFSET_START_SEARCH.Signature)
                throw new OffsetException("Unable to find Inventory offset, corrupt save?");

            inventory.ItemCount = mainReader.ReadItemBits<UInt16>(InventoryOffsets.OFFSET_ITEM_COUNT);
            Logger.WriteSection(mainReader, InventoryOffsets.OFFSET_ITEM_COUNT.BitLength, $"Item Count: {inventory.ItemCount}");

            inventory.ItemList = new HashSet<ItemStructure>();
            for(int i=0;i<inventory.ItemCount;i++)
            {
                Logger.WriteBeginSection($"[Begin reading item #{i}]");
                inventory.ItemList.Add(new ItemStructure(mainReader, i));
                Logger.WriteEndSection($"[End reading item #{i}]");
            }
            return inventory;
        }

        public static int InventoryOffset = -1;
        public static int EndInventoryOffset = -1;
        public static int EndCorpseOffset = -1;
        public static int EndOfMercenaryOffset = -1;
        public static void FindInventoryOffsetInBytes(BitwiseBinaryReader mainReader)
        {
            mainReader.SetBytePosition(InventoryOffsets.OFFSET_START_SEARCH.Offset);
            while (mainReader.PeekBits(16).ToString(16, Endianness.BigEndian) != InventoryOffsets.OFFSET_START_SEARCH.Signature) mainReader.SkipBytes(1);

            InventoryOffset = mainReader.bitPosition;
            Logger.WriteSection(mainReader, 0, $"Inventory Offset found: {InventoryOffset / 8} | {InventoryOffset}");
            mainReader.SetBitPosition(0);
        }

        public bool WriteInventory(BitwiseBinaryWriter writer)
        {
            writer.WriteBits(InventoryOffsets.OFFSET_START_SEARCH.Signature.ToBits());
            writer.WriteBits(ItemCount.ToBits((uint)InventoryOffsets.OFFSET_ITEM_COUNT.BitLength));

            for(int i=0;i<ItemCount;i++)
            {
                ItemList.ElementAt(i).Write(writer);
            }

            return true;
        }
    }
}
